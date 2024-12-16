use futures::Stream;
use std::{pin::Pin, time::Duration};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncSeekExt},
    time,
};

use crate::{
    utils::{LOG_SUBDIR, SUPERVISORS_LOG_FILENAME},
    MonocoreResult,
};

use super::Orchestrator;

type BoxedStream = Pin<Box<dyn Stream<Item = MonocoreResult<String>> + Send>>;

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Orchestrator {
    /// View logs for a specific service
    ///
    /// # Arguments
    /// * `service_name` - Name of the service to view logs for
    /// * `lines` - Optional number of lines to show from the end
    /// * `follow` - Whether to continuously follow the log output
    pub async fn view_logs(
        &self,
        service_name: Option<&str>,
        lines: Option<usize>,
        follow: bool,
    ) -> MonocoreResult<BoxedStream> {
        // Get log directory from home directory
        let log_dir = self.home_dir.join(LOG_SUBDIR);

        // Ensure log directory exists
        if !fs::try_exists(&log_dir).await? {
            let msg = "No logs found".to_string();
            return Ok(Box::pin(futures::stream::once(futures::future::ready(Ok(
                msg,
            )))));
        }

        let log_path = if let Some(service_name) = service_name {
            let log_path = log_dir.join(format!("{}.stdout.log", service_name));

            // Check if log file exists
            if !fs::try_exists(&log_path).await? {
                let msg = format!("No logs found for service '{}'", service_name);
                return Ok(Box::pin(futures::stream::once(futures::future::ready(Ok(
                    msg,
                )))));
            }

            log_path
        } else {
            let log_path = log_dir.join(SUPERVISORS_LOG_FILENAME);

            // Ensure supervisor log file exists
            if !fs::try_exists(&log_path).await? {
                let msg = "Supervisor logs not found".to_string();
                return Ok(Box::pin(futures::stream::once(futures::future::ready(Ok(
                    msg,
                )))));
            }

            log_path
        };

        // Read initial content
        let content = fs::read_to_string(&log_path).await?;
        let content = if let Some(n) = lines {
            let lines: Vec<&str> = content.lines().collect();
            let start = if lines.len() > n { lines.len() - n } else { 0 };
            lines[start..].join("\n")
        } else {
            content
        };

        // Ensure content ends with newline
        let initial_content = if content.ends_with('\n') {
            content
        } else {
            content + "\n"
        };

        if !follow {
            return Ok(Box::pin(futures::stream::once(futures::future::ready(Ok(
                initial_content,
            )))));
        }

        // For follow mode, create a stream that continuously reads the file
        let log_path_clone = log_path.clone();
        let stream = async_stream::stream! {
            // Send initial content
            yield Ok(initial_content);

            let mut last_size = fs::metadata(&log_path_clone).await?.len();
            let mut interval = time::interval(Duration::from_millis(100));

            loop {
                interval.tick().await;

                // Check if file still exists
                if !fs::try_exists(&log_path_clone).await? {
                    break;
                }

                let metadata = fs::metadata(&log_path_clone).await?;
                let current_size = metadata.len();

                if current_size > last_size {
                    // Read only the new content
                    let mut file = fs::File::open(&log_path_clone).await?;
                    file.seek(std::io::SeekFrom::Start(last_size)).await?;
                    let mut new_content = String::new();
                    file.read_to_string(&mut new_content).await?;
                    last_size = current_size;
                    yield Ok(new_content);
                }
            }
        };

        Ok(Box::pin(stream))
    }
}
