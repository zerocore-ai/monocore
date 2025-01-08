# `monobase`

## Table of Contents
- [Introduction](#introduction)
- [Building on `monofs`](#building-on-monofs)
- [Architecture](#architecture)
  - [Bases as Directories](#bases-as-directories)
  - [Raft for Linearizability](#raft-for-linearizability)
  - [Local-First Strategies via CRDTs](#local-first-strategies-via-crdts)
  - [Multi-Raft Cross-Partition Atomicity](#multi-raft-cross-partition-atomicity)
- [Features](#features)
- [Use Cases](#use-cases)
- [Conclusion](#conclusion)

---

## Introduction

`monobase` is a database abstraction layer built on top of `monofs`. It leverages `monofs`’s content-addressable, versioned file system and its strong-consistency plus local-first merging capabilities. In `monobase`, a directory corresponds to a "base" (database), and files represent tables, indexes, and other schema objects. This mapping transforms the complexity of distributed databases into manageable file system operations.

## Building on `monofs`

`monobase` inherits from `monofs`:

- **IPLD-based Content Addressability:** Each database object (table, index) is stored as IPLD nodes with cryptographic integrity guarantees.
- **Versioning:** The natural versioning of `monofs` facilitates snapshotting databases, rolling back changes, and auditing histories.
- **Multi-Raft Scalability:** Distribute data across multiple Raft groups, ensuring that no single leader becomes a performance bottleneck.
- **Local-First / CRDT-Inspired Merges:** Apply local-first updates to parts of the database for flexible and responsive editing experiences.

## Architecture

### Bases as Directories

In `monobase`, a "base" is simply a directory within `monofs`. Inside it:

- **Tables & Indexes as Files:** Complex database structures map cleanly to files and sub-directories.
- **Hierarchical Namespaces:** Organize data intuitively, using directory hierarchies to represent logical groupings or per-user datasets.

### Raft for Linearizability

`monobase` relies on Raft groups to ensure linearizable reads and writes for parts of the database requiring strict consistency. With leader leases for local reads, queries and transactions can be served efficiently, maintaining correctness without undue latency.

### Local-First Strategies via CRDTs

Not all data needs strict linearizability. `monobase` can apply local-first, CRDT-like strategies for certain subsets of the database:

- **Optimistic Updates:** Applications can perform updates locally without immediate coordination.
- **Conflict-Free Merging:** Changes sync later without conflicts, enabling responsive user interactions and collaborative editing.

This hybrid approach allows critical data to remain strongly consistent while other data can favor availability and low-latency updates.

### Multi-Raft Cross-Partition Atomicity

`monobase` achieves cross-partition atomicity through a unique approach:

- **Temporary Consolidation into a New Raft Group:** Rather than using traditional two-phase commit or optimistic concurrency control, `monobase` temporarily moves the relevant partitions into a dedicated "transaction directory," forming a new Raft group.
- **Original Leaders as Participants:** The leaders of the involved partitions join this temporary Raft group. They continue sending heartbeats to their followers but pause log replication of their partition data.
- **Shared Transaction Log for Approval:** The new Raft group maintains a log specifically for transaction approval rather than for the data itself. The temporary leader reads from each partition, transforms the data, and writes changes into this shared log.
- **Commit and Release:** Once the transaction is approved, changes are applied. The partitions then return to their original configuration, resuming normal Raft operation.

This approach sidesteps standard multi-partition commit protocols, allowing `monobase` to orchestrate atomic operations across partitions in a controlled, dynamic manner.

## Features

- **Base as a Database:** Directories map directly to logical databases, simplifying understanding and maintenance.
- **Tables, Indexes, Schemas as Files:** Database entities naturally represented as file system objects.
- **Strong Consistency & Local-First Blend:** Combine Raft-backed linearizability for critical data with CRDT-style merging for flexible updates.
- **Multi-Raft Scalability & Adaptive Partitioning:** Distribute workload across multiple consensus groups, adjusting partitioning as needed.
- **Unique Cross-Partition Atomicity Approach:** Temporarily consolidate partitions into a new Raft group for transaction approval, avoiding traditional commit protocols.
- **Rich Versioning & Auditing:** Leverage `monofs`’s immutable DAG for snapshots, lineage tracking, and recovery.

## Use Cases

- **Globally Distributed Databases:** Deploy `monobase` across regions, ensuring strong consistency where needed and local-first collaboration elsewhere.
- **Analytics & Logging:** Treat logs or analytical tables as verifiable files within a scalable, versioned filesystem.
- **Complex CMS & Document Stores:** Represent documents, hierarchies, and indexes as files and directories, simplifying backup, restore, and replication.
- **Collaborative Editing Applications:** Combine linearizable operations for mission-critical data with local-first merges for user-generated content.

## Conclusion

`monobase` extends the power of `monofs` into the realm of distributed databases, blending linearizability, local-first updates, adaptive partitioning, and a novel approach to cross-partition atomicity. By utilizing directories as databases and files as tables, `monobase` offers developers a familiar yet powerful paradigm for building scalable, consistent, and flexible distributed data systems.
