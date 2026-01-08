### Changed
- **Scalpel Configuration**: Removed hardcoded default model from `src/clients/local.rs`. The `SURR_SCALPEL_MODEL` environment variable is now **mandatory**. This prevents silent failures/mismatches by forcing explicit configuration in `.env`.
- **Documentation**: Added Scalpel configuration section to `.env.example`.