//! This example demonstrates using FlatFsStore for persistent content-addressed storage.
//!
//! The example shows how to:
//! - Initialize and configure a FlatFsStore
//! - Store and retrieve structured data
//! - Work with CIDs and content addressing
//! - Handle metadata and content persistence
//!
//! Operations demonstrated:
//! 1. Setting up a persistent store directory
//! 2. Creating and storing structured content
//! 3. Managing content metadata
//! 4. Retrieving and displaying stored content
//! 5. Working with CIDs and references
//! 6. Handling store statistics and codecs
//!
//! To run the example:
//! ```bash
//! cargo run --example flatfs_store -- /path/to/store
//! ```

use anyhow::Result;
use clap::Parser;
use monofs::store::FlatFsStoreDefault;
use monoutils_store::ipld::cid::Cid;
use monoutils_store::{IpldReferences, IpldStore};
use serde::{Deserialize, Serialize};
use serde_json;
use std::{collections::BTreeMap, path::PathBuf, sync::LazyLock};

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Initialize logging
    tracing_subscriber::fmt::init();

    // Create store directory and blocks subdirectory
    let blocks_path = args.path.join("blocks");
    tokio::fs::create_dir_all(&blocks_path).await?;
    println!("\nUsing store directory: {}\n", args.path.display());

    // Initialize the store with blocks directory
    let store = FlatFsStoreDefault::new(blocks_path.to_str().unwrap());

    // Path to head CID file
    let head_path = args.path.join("head");

    // Try to read existing head CID
    let head_cid = if head_path.exists() {
        let head_contents = tokio::fs::read_to_string(&head_path).await?;
        Some(Cid::try_from(head_contents.trim())?)
    } else {
        None
    };

    // If no head CID exists, create and store initial data
    let cid = if head_cid.is_none() {
        println!("No existing head CID found. Creating initial data...");

        // Create a new store with metadata and literary content
        let mut store_data = KeyValueStore::new();

        // Add metadata as direct content
        let metadata = Content::new(
            serde_json::json!({
                "name": "Literary Quotes Collection",
                "version": "1.0.0",
                "description": "A collection of famous literary quotes and excerpts",
                "curator": "FlatFs Example Store",
            })
            .to_string(),
        );

        let metadata_cid = store.put_node(&metadata).await?;
        store_data.insert("metadata", metadata_cid);

        // Store each text content and add its CID to the store
        for (key, content) in LITERARY_TEXTS.iter() {
            let content_cid = store.put_node(content).await?;
            store_data.insert(*key, content_cid);
        }

        // Store the index and get its CID
        let cid = store.put_node(&store_data).await?;
        println!("Stored initial data with CID: {}", cid);

        // Save the CID to head file
        tokio::fs::write(&head_path, cid.to_string()).await?;

        cid
    } else {
        let cid = head_cid.unwrap();
        println!("Found existing head CID: {}", cid);
        cid
    };

    // Retrieve and display the stored data
    println!("\nRetrieving stored data:");
    let store_data: KeyValueStore = store.get_node(&cid).await?;

    // Display metadata first
    println!("\nMetadata:");
    if let Some(metadata_cid) = store_data.data.get("metadata") {
        let metadata: Content = store.get_node(metadata_cid).await?;
        let metadata_json: serde_json::Value = serde_json::from_str(&metadata.text)?;
        for (key, value) in metadata_json.as_object().unwrap() {
            println!("  {} = {}", key, value);
        }
    }

    // Display literary content with proper formatting
    println!("\nLiterary Content:");
    for (key, content_cid) in &store_data.data {
        if key != "metadata" {
            println!("\n=== {} ===", key.replace('_', " ").to_uppercase());
            let content: Content = store.get_node(content_cid).await?;

            // Word wrap the text at 80 characters
            let mut current_line = String::new();
            for word in content.text.split_whitespace() {
                if current_line.len() + word.len() + 1 > 80 {
                    println!("  {}", current_line);
                    current_line = word.to_string();
                } else {
                    if current_line.is_empty() {
                        current_line = word.to_string();
                    } else {
                        current_line = format!("{} {}", current_line, word);
                    }
                }
            }
            if !current_line.is_empty() {
                println!("  {}", current_line);
            }
        }
    }

    // Display store statistics
    println!("\nStore Statistics:");
    println!("  Total blocks: {}", store.get_block_count().await?);
    println!("  Is empty: {}", store.is_empty().await?);
    println!("  Head CID: {}", cid);
    println!("  Number of content blocks: {}", store_data.data.len());

    let supported_codecs = store.get_supported_codecs().await;
    println!("  Supported codecs:");
    for codec in supported_codecs {
        println!("    - {:?}", codec);
    }

    if let Some(max_size) = store.get_node_block_max_size().await? {
        println!("  Max node block size: {} bytes", max_size);
    }

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Example demonstrating FlatFsStore operations with persistence
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Path to the store directory
    #[arg(
        help = "Path to the store directory. Will be created if it doesn't exist. Blocks are stored in $path/blocks and head CID in $path/head"
    )]
    path: PathBuf,
}

/// A simple key-value store that implements IpldReferences where values are CIDs
#[derive(Debug, Serialize, Deserialize)]
struct KeyValueStore {
    data: BTreeMap<String, Cid>,
}

/// Content wrapper for storing text content
#[derive(Debug, Serialize, Deserialize)]
struct Content {
    text: String,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Content {
    fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

impl KeyValueStore {
    fn new() -> Self {
        Self {
            data: BTreeMap::new(),
        }
    }

    fn insert(&mut self, key: impl Into<String>, cid: Cid) {
        self.data.insert(key.into(), cid);
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl IpldReferences for Content {
    fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a> {
        Box::new(std::iter::empty())
    }
}

impl IpldReferences for KeyValueStore {
    fn get_references<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Cid> + Send + 'a> {
        // Return all CIDs in the values
        Box::new(self.data.values())
    }
}

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// Literary content stored as individual Content objects
static LITERARY_TEXTS: LazyLock<BTreeMap<&'static str, Content>> = LazyLock::new(|| {
    let mut texts = BTreeMap::new();

    // Famous opening lines
    texts.insert(
        "moby_dick_opening",
        Content::new("Call me Ishmael. Some years ago—never mind how long precisely—having little or no money in my purse, and nothing particular to interest me on shore, I thought I would sail about a little and see the watery part of the world.")
    );

    texts.insert(
        "pride_and_prejudice_opening",
        Content::new("It is a truth universally acknowledged, that a single man in possession of a good fortune, must be in want of a wife. However little known the feelings or views of such a man may be on his first entering a neighbourhood, this truth is so well fixed in the minds of the surrounding families, that he is considered the rightful property of some one or other of their daughters.")
    );

    texts.insert(
        "tale_of_two_cities_opening",
        Content::new("It was the best of times, it was the worst of times, it was the age of wisdom, it was the age of foolishness, it was the epoch of belief, it was the epoch of incredulity, it was the season of Light, it was the season of Darkness, it was the spring of hope, it was the winter of despair, we had everything before us, we had nothing before us, we were all going direct to Heaven, we were all going direct the other way.")
    );

    // Famous soliloquies
    texts.insert(
        "hamlet_soliloquy",
        Content::new("To be, or not to be, that is the question: Whether 'tis nobler in the mind to suffer The slings and arrows of outrageous fortune, Or to take Arms against a Sea of troubles, And by opposing end them: to die, to sleep No more; and by a sleep, to say we end The heart-ache, and the thousand natural shocks That flesh is heir to? 'Tis a consummation Devoutly to be wished.")
    );

    texts.insert(
        "macbeth_soliloquy",
        Content::new("Tomorrow, and tomorrow, and tomorrow, Creeps in this petty pace from day to day, To the last syllable of recorded time; And all our yesterdays have lighted fools The way to dusty death. Out, out, brief candle! Life's but a walking shadow, a poor player That struts and frets his hour upon the stage And then is heard no more. It is a tale Told by an idiot, full of sound and fury, Signifying nothing.")
    );

    // Famous poems
    texts.insert(
        "the_raven",
        Content::new("Once upon a midnight dreary, while I pondered, weak and weary, Over many a quaint and curious volume of forgotten lore— While I nodded, nearly napping, suddenly there came a tapping, As of some one gently rapping, rapping at my chamber door. ''Tis some visitor,' I muttered, 'tapping at my chamber door— Only this and nothing more.'")
    );

    texts.insert(
        "ozymandias",
        Content::new("I met a traveller from an antique land Who said: 'Two vast and trunkless legs of stone Stand in the desert. Near them, on the sand, Half sunk, a shattered visage lies, whose frown, And wrinkled lip, and sneer of cold command, Tell that its sculptor well those passions read Which yet survive, stamped on these lifeless things, The hand that mocked them and the heart that fed.'")
    );

    // Modern literature
    texts.insert(
        "hundred_years_of_solitude",
        Content::new("Many years later, as he faced the firing squad, Colonel Aureliano Buendía was to remember that distant afternoon when his father took him to discover ice. At that time Macondo was a village of twenty adobe houses, built on the bank of a river of clear water that ran along a bed of polished stones, which were white and enormous, like prehistoric eggs.")
    );

    texts.insert(
        "the_metamorphosis",
        Content::new("As Gregor Samsa awoke one morning from uneasy dreams he found himself transformed in his bed into a gigantic insect. He was lying on his hard, as it were armor-plated, back and when he lifted his head a little he could see his dome-like brown belly divided into stiff arched segments on top of which the bed quilt could hardly keep in position and was about to slide off completely.")
    );

    texts
});
