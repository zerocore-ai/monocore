# monoutils-cryptdag

Inspired by [WNFS private partition spec](https://github.com/wnfs-wg/spec/blob/main/spec/private-wnfs.md).
`cryptdag` is meant to be a secure and flexible way to generate keys for data structures that can be represented as trees or DAGs.

### Key Ideas

- Parent Temporal Key encrypts child Temporal Key
- Parent Snapshot Key encrypts child Snapshot Key
- Temporal Key is derived from the Ratchet and the key is expected to change with every revision.
- Snapshot Key is derived from the Temporal Key
- Parent Challenge
- Child Ratchet is created from a new seed

### Use Cases

- File systems
- Databases
- Hierarchical Deterministic Keys like device keys
- Note taking app
- Version control systems
- Messaging
