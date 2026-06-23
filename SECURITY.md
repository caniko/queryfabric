# Security

Security-sensitive behavior such as query execution, authorization, and data
access policy remains outside QueryFabric core.

For the current compiler and federation attack surface, see the mdBook chapter
at `docs/src/project/threat-model.md`.

Security reports for QueryFabric should focus on:

- SQL or artifact generation bugs
- placeholder handling and parameter propagation
- incorrect capability classification or unsafe backend emission
- provenance or schema metadata mismatches that could mislead downstream users

Until standalone governance is fully established, coordinate disclosures through
the QueryFabric maintainers.
