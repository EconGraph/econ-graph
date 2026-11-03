# Patched implementations behind legacy dependency APIs

The current Iceberg/DataFusion dependency graph still requires `zstd ^0.13` and
`quick-xml ^0.39`, even though their fixes ship on later minor releases. These two local
compatibility packages satisfy the current consumers' dependency requirements by re-exporting the **actual**
patched registry implementations. They contain no copied compression/XML implementation,
unsafe code, build script, or alternative parser. Every legacy Cargo feature forwards to its
matching feature in the patched implementation.

| Local compatibility API | Actual registry implementation | Reason |
| --- | --- | --- |
| `zstd 0.13.3` | `zstd 0.14.3`, with `zstd-safe 8.2.0` | Iceberg, Avro, Arrow IPC, Parquet and DataFusion still request the older API line. |
| `quick-xml 0.39.4` | `quick-xml 0.42.0` | OpenDAL core and its S3 service still request the older API line. |

These internal facades cover APIs used by the current compiled consumers, rather than promising
complete compatibility with every method from the older release. For example quick-xml 0.42
changes text-decoding APIs that OpenDAL does not call. A new consumer must compile and pass its
own behavior checks before using the facade.

The local versions describe which consumer dependency requirement is satisfied; they do not identify
an included upstream source payload. Their manifests explicitly describe the facade, are not
publishable, and refer to exact patched versions with normal crates.io checksums in Cargo.lock.
The facades are excluded from workspace membership so workspace-wide `--all-features` does not
activate their otherwise unused bindgen/experimental features. They remain normal path
dependencies in the complete audited graph. The upstream packages remain distinct packages in the resolved dependency graph and are scanned
normally by cargo-audit and cargo-deny. There are no advisory ignores, source exclusions, or
scanner filters for these packages.

Because registry vulnerability matching cannot establish the meaning of a local facade,
`backend/scripts/check_security_adapters.py` validates the local code explicitly before every
security audit. It refuses any source beyond `forbid(unsafe_code)` plus the direct re-export,
any extra file/build script/dependency, changes to feature forwarding, missing patched registry
packages/checksums, a return of the old implementation, or advisory waivers for these fixes.
Mutation tests exercise those refusal paths. CI audits the validated graph after that check;
local packages are not used to hide an unpatched implementation.

The build and live compatibility checks must cover the real consumer graph, including actual
Iceberg/DataFusion writes, OpenDAL S3 requests, independent PyIceberg/DuckDB reads and restoration.
A successful facade-only compilation is insufficient. Keep the normal release migration and
storage cutover gates unchanged.

Tokio is upgraded normally to `1.54.1`, which fixes the AsyncFd I/O safety advisory; it needs no
facade. Existing h2 policy is unchanged.

Remove each facade when the official upstream dependency graph accepts the patched API line.
An upstream upgrade is preferable to maintaining a local compatibility layer. If a later release
breaks API compatibility, update the consuming upstream crate or maintain an explicitly reviewed
adapter; do not copy vulnerable implementation code into these facades or waive its advisories.

Fix references:

- [zstd context reuse](https://rustsec.org/advisories/RUSTSEC-2026-0391.html)
- [zstd-safe streaming decompression](https://rustsec.org/advisories/RUSTSEC-2026-0403.html)
- [zstd-safe seekable decompression](https://rustsec.org/advisories/RUSTSEC-2026-0398.html)
- [quick-xml namespace allocation](https://rustsec.org/advisories/RUSTSEC-2026-0195.html)
- [quick-xml duplicate attributes](https://rustsec.org/advisories/RUSTSEC-2026-0194.html)
- [Tokio AsyncFd I/O safety](https://rustsec.org/advisories/RUSTSEC-2026-0386.html)
