# `monofs`

## Table of Contents
- [Introduction](#introduction)
- [Background and Motivation](#background-and-motivation)
- [Architecture](#architecture)
  - [IPLD and the Merkle DAG](#ipld-and-the-merkle-dag)
  - [Raft for High Consistency and Leader Leases](#raft-for-high-consistency-and-leader-leases)
  - [CRDT and Local-First Updates](#crdt-and-local-first-updates)
- [Features](#features)
- [Distributed Semantics and Atomicity](#distributed-semantics-and-atomicity)
- [Use Cases](#use-cases)
- [Conclusion](#conclusion)

---

## Introduction

`monofs` is a distributed, content-addressable file system inspired by the public layer of the Web-Native File System (WNFS). It simplifies the WNFS approach by excluding private partitions, making the system leaner while retaining key benefits like IPLD-based storage, Raft-driven consensus, and local-first merging capabilities.

## Background and Motivation

WNFS couples a public and a private file system under a unified content-addressable, Merkle DAG structure. However, the private layer adds complexity—encryption, key management, and permission handling. `monofs` focuses solely on the public dimension, aiming to provide a straightforward, highly scalable file system. By deferring encryption and privacy to upper layers, `monofs` remains flexible, easier to implement, and more efficient.

## Architecture

### IPLD and the Merkle DAG

`monofs` leverages IPLD (InterPlanetary Linked Data) to represent files and directories as nodes in a Merkle DAG. IPLD guarantees:

- **Content Addressability:** References by content hash, ensuring deduplication and data integrity.
- **Merkle DAG Structure:** Immutable links between nodes, enabling verifiable and tamper-evident data structures.

These properties simplify versioning and snapshotting—each change in `monofs` creates a new Merkle DAG root, facilitating easy auditing, rollback, and historical queries.

### Raft for Strong Consistency

`monofs` employs the Raft consensus protocol to maintain linearizable reads and writes within each partition. Raft ensures:

- **Linearizable Operations:** Strict ordering of updates makes the file system behave as if there is a single authoritative timeline.
- **Leader Leases for Local Reads:** Instead of forwarding reads to the leader for every request, leader leases permit local reads without extra round trips, reducing latency and improving throughput.

Multiple Raft groups (multi-raft) can manage different subsets of the file system, distributing load and preventing a single consensus bottleneck.

### CRDT and Local-First Updates

While Raft delivers strong consistency, `monofs` also supports local-first strategies inspired by CRDTs:

- **Extended File Attributes:** Applications can mark directories as being under CRDT-like conflict resolution.
- **Local-First Editing:** Nodes can apply updates optimistically and merge changes later, minimizing user-facing latency.
- **Conflict-Free Merging:** When nodes reconcile, updates merge seamlessly, ensuring eventual consistency and preserving all changes.

This dual mode—strict consistency via Raft and flexible merging via CRDT-like strategies—allows `monofs` to cater to various consistency requirements.

### Multi-Raft Partitioning

`monofs` uses multiple Raft groups (multi-raft) to segment the file system. Each group manages a subset of the data, distributing load and preventing a single consensus bottleneck. Partitioning may be guided by heuristics such as:

- **Workload-Based Splitting:** If a directory contains multiple large files frequently written to, these files might be split into separate Raft groups for better parallelism.
- **Adaptive Partitioning Strategies:** As usage patterns evolve, partitions could be reorganized based on data size, write frequency, or other dynamic factors.

While the exact heuristics for partitioning are not finalized, the architecture is designed to accommodate flexible, data-driven strategies to improve scalability and performance.

## Features

- **Content-Addressability & IPLD:** Efficient deduplication, integrity checks, and easy versioning.
- **Strong Consistency via Raft:** Linearizable operations ensure predictable behavior and correctness.
- **Local-First Merging (CRDT-Inspired):** Reduce conflict resolution complexity and improve latency.
- **Multi-Raft Scalability:** Parallelize consensus workloads, adaptively partition data, and improve throughput.
- **Versioning & Snapshotting:** Track and revert changes easily, audit historical states, and maintain a robust data lineage.

## Distributed Semantics and Atomicity

`monofs` provides the building blocks for managing files across multiple Raft partitions. While it doesn't solve every cross-partition transactional complexity, its architecture supports higher-level protocols and structures that can implement atomic operations and distributed transactions. By leveraging versioned DAGs and a robust consensus layer, developers can implement sophisticated transaction managers or lightweight coordination protocols atop `monofs`.

## Use Cases

- **Distributed File Storage:** Scale out file management, keeping data consistent and verifiable.
- **Global Content Distribution:** Serve immutable, verifiable content worldwide with minimal complexity.
- **Collaboration and Editing Tools:** Allow concurrent editing with local-first responsiveness and automatic merging.
- **Version-Controlled Repositories:** Manage code, documents, or media with built-in versioning and snapshotting capabilities.

## Conclusion

`monofs` provides a strong foundation for distributed, verifiable, and flexible file storage. By marrying IPLD’s Merkle DAG structure with Raft’s strong consistency and CRDT-inspired local-first merging, `monofs` achieves a powerful balance of correctness, scalability, and usability. Developers can build on `monofs` to create sophisticated systems that benefit from easy versioning, scalable consensus, and intuitive data management without incurring the complexity of private file system layers.
