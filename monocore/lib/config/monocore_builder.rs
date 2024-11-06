use crate::config::{Group, Monocore, Service};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Builder for the Monocore configuration
#[derive(Default)]
pub struct MonocoreBuilder {
    services: Vec<Service>,
    groups: Vec<Group>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl MonocoreBuilder {
    /// Sets the services for the configuration
    pub fn services(mut self, services: impl IntoIterator<Item = Service>) -> Self {
        self.services = services.into_iter().collect();
        self
    }

    /// Sets the groups for the configuration
    pub fn groups(mut self, groups: impl IntoIterator<Item = Group>) -> Self {
        self.groups = groups.into_iter().collect();
        self
    }

    /// Builds the Monocore configuration, validating it in the process
    pub fn build(self) -> crate::MonocoreResult<Monocore> {
        let monocore = Monocore {
            services: self.services,
            groups: self.groups,
        };

        // Validate the configuration before returning
        monocore.validate()?;

        Ok(monocore)
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Service;

    #[test]
    fn test_monocore_builder_minimal() {
        let monocore = Monocore::builder()
            .services(vec![])
            .groups(vec![])
            .build()
            .unwrap();

        assert!(monocore.services.is_empty());
        assert!(monocore.groups.is_empty());
    }

    #[test]
    fn test_monocore_builder_with_service() {
        let service = Service::builder_default()
            .name("test-service")
            .command("./test")
            .build();

        let monocore = Monocore::builder()
            .services(vec![service])
            .groups(vec![])
            .build()
            .unwrap();

        assert_eq!(monocore.services.len(), 1);
        assert!(monocore.groups.is_empty());
    }

    #[test]
    fn test_monocore_builder_with_group() {
        let group = Group::builder().name("test-group").build();

        let monocore = Monocore::builder()
            .services(vec![])
            .groups(vec![group])
            .build()
            .unwrap();

        assert!(monocore.services.is_empty());
        assert_eq!(monocore.groups.len(), 1);
    }

    #[test]
    fn test_monocore_builder_validation_failure() {
        // Create two services with the same name to trigger validation error
        let service1 = Service::builder_default()
            .name("test-service")
            .command("./test")
            .build();

        let service2 = Service::builder_default()
            .name("test-service") // Same name as service1
            .command("./test")
            .build();

        let result = Monocore::builder()
            .services(vec![service1, service2])
            .groups(vec![])
            .build();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Duplicate service name"));
    }
}
