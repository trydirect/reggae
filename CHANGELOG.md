# Changelog

All notable changes to this project will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
- **Gandi provider** — full `DomainRegistrar` implementation using the Gandi REST v5 API
  - Domain API (`/v5/domain/`) for registration, renewal, transfer, domain info, and nameserver management
  - LiveDNS API (`/v5/livedns/`) for DNS record CRUD (A, AAAA, CNAME, MX, TXT, NS, SRV, CAA, ALIAS)
  - Bearer auth via Personal Access Token (PAT)
  - Sandbox support (`api.sandbox.gandi.net`) toggled by `sandbox: true` in config
  - RRset-based record IDs (`name/TYPE`) to match Gandi's zone model; multi-value rrsets expand into indexed records (`www/A[1]`)
  - MX wire-format conversion between Gandi's `"10 mail.example.com."` and the trait's `content`+`priority` fields
  - Config keys: `providers.gandi.personal_access_token`, `providers.gandi.sandbox`
  - Env vars: `DM_PROVIDERS_GANDI_PERSONAL_ACCESS_TOKEN`, `DM_PROVIDERS_GANDI_SANDBOX`
  - 20 unit tests covering all trait methods, record-ID parsing, rrset expansion, and MX roundtrip

- **Name.com provider** — full `DomainRegistrar` implementation using the Name.com REST v4 API
  - JSON REST API with HTTP Basic auth (username + API token)
  - Sandbox support (`api.dev.name.com`) toggled by `sandbox: true` in config
  - Paginated DNS record listing
  - Config keys: `providers.namecom.username`, `providers.namecom.api_token`, `providers.namecom.sandbox`
  - Env vars: `DM_PROVIDERS_NAMECOM_USERNAME`, `DM_PROVIDERS_NAMECOM_API_TOKEN`, `DM_PROVIDERS_NAMECOM_SANDBOX`
  - 18 unit tests covering all trait methods and error paths

- **Dynadot provider** — full `DomainRegistrar` implementation using the stable Dynadot XML API v3
  - All requests via HTTP GET + query params; XML responses parsed with `roxmltree`
  - Full-set DNS replacement model (same as Namecheap): create/update/delete fetch the current record set, modify, and push atomically via `set_dns2`
  - Synthetic record IDs encoded as `TYPE:subdomain` (e.g. `A:@`, `MX:@`)
  - Sandbox support (`api-sandbox.dynadot.com`) toggled by `sandbox: true` in config
  - REST JSON beta API available behind `use_rest_api: true` flag (not yet implemented)
  - Expiry date decoded from Dynadot's Unix-timestamp-in-milliseconds format
  - Config keys: `providers.dynadot.api_key`, `providers.dynadot.sandbox`
  - Env vars: `DM_PROVIDERS_DYNADOT_API_KEY`, `DM_PROVIDERS_DYNADOT_SANDBOX`
  - 11 unit tests

- **IONOS provider** — DNS management + domain info via Hetzner-style zone-based REST API
  - Auth via composite `X-API-Key: prefix.secret` header
  - Zone-based DNS: resolves zone ID from `/dns/v1/zones` then operates on records
  - `check_availability`, `register_domain`, `renew_domain`, `transfer_domain` return `Error::Unsupported` (not available in IONOS API)
  - `get_domain_info` and `set_nameservers` use the `/domains/v1/` sub-API
  - Config keys: `providers.ionos.api_prefix`, `providers.ionos.api_secret`
  - Env vars: `DM_PROVIDERS_IONOS_API_PREFIX`, `DM_PROVIDERS_IONOS_API_SECRET`
  - 8 unit tests

### Planned
- **Hetzner DNS provider** (DNS-only) — Hetzner Cloud DNS API (`api.hetzner.cloud/v1/dns/`) with Bearer token auth; zone-based DNS CRUD for existing Hetzner zones. Domain registration/availability/renewal are not available via any Hetzner REST API (their Domain Registration Robot is email-only).

## [0.1.0] — 2025-05-01

### Added
- Initial release with four providers: **Porkbun**, **Cloudflare**, **GoDaddy**, **Namecheap**
- Unified `DomainRegistrar` trait covering: availability check, register, renew, transfer, domain info, DNS CRUD, nameserver get/set, pricing
- CLI commands: `check`, `register`, `dns` (list/add/update/delete), `ns` (get/set), `info`, `renew`, `transfer`, `pricing`, `schedule`, `run-check`, `completion`
- YAML + environment-variable configuration with `DM_*` prefix overrides
- Background expiry scheduler (daemon and one-shot modes) with stdout and webhook notifiers
- JSON and table output formats
- Shell completion generation for bash, zsh, fish
- Multi-arch release workflow (GitHub Actions)
- curl installer script
