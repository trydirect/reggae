# reggae

**Cross-registrar domain manager CLI** — manage domains and DNS records across Porkbun, Cloudflare, GoDaddy, Namecheap and more from a single command-line tool.

## Installation

```bash
curl -sSL https://raw.githubusercontent.com/trydirect/reggae/main/installer.sh | bash
```

Or build from source:

```bash
cargo install --path .
```

## Quick start

```bash
# Set credentials via environment variables
export DM_PROVIDERS_PORKBUN_API_KEY=pk1_...
export DM_PROVIDERS_PORKBUN_SECRET_API_KEY=sk1_...

# Check availability
reggae check example.com

# Register a domain with an A record
reggae register --domain example.com --A 198.51.100.1

# List DNS records
reggae dns list --domain example.com

# Add a DNS record
reggae dns add --domain example.com --record-type TXT --name @ --content "v=spf1 include:spf.example.com ~all"

# Get pricing
reggae pricing com

# Get domain info
reggae info example.com
```

## Configuration

Credentials are **never** passed as CLI arguments. Use environment variables (highest priority) or `config.yaml`.

```bash
cp config.yaml.example config.yaml
$EDITOR config.yaml
```

| Environment variable | Config key | Description |
|---|---|---|
| `DM_DEFAULT_PROVIDER` | `default_provider` | Default registrar |
| `DM_PROVIDERS_PORKBUN_API_KEY` | `providers.porkbun.api_key` | Porkbun API key |
| `DM_PROVIDERS_PORKBUN_SECRET_API_KEY` | `providers.porkbun.secret_api_key` | Porkbun secret |
| `DM_PROVIDERS_CLOUDFLARE_API_TOKEN` | `providers.cloudflare.api_token` | Cloudflare token |
| `DM_PROVIDERS_GODADDY_API_KEY` | `providers.godaddy.api_key` | GoDaddy key |
| `DM_PROVIDERS_GODADDY_API_SECRET` | `providers.godaddy.api_secret` | GoDaddy secret |
| `DM_SCHEDULER_ENABLED` | `scheduler.enabled` | Enable expiry daemon |
| `DM_SCHEDULER_EXPIRY_LEAD_DAYS` | `scheduler.expiry_lead_days` | Notify N days before expiry |

## Expiry notifications

```bash
# Daemon mode (runs continuously)
reggae schedule

# One-shot (use with cron or systemd timer)
reggae run-check
```

Add domains to monitor in `config.yaml`:

```yaml
scheduler:
  enabled: true
  expiry_lead_days: 30
  domains:
    - example.com
    - mysite.io
  notification:
    type: webhook
    webhook_url: https://hooks.example.com/notify
```

## Providers

| Provider | Status | Auth |
|---|---|---|
| Porkbun | ✅ Full | `api_key` + `secret_api_key` |
| Cloudflare | ✅ DNS | `api_token` (Bearer) |
| GoDaddy | 🔧 Skeleton | `api_key:api_secret` |
| Namecheap | 🔧 Skeleton | XML API — roadmap |

## Output formats

```bash
reggae dns list --domain example.com --output table   # default
reggae dns list --domain example.com --output json    # machine-readable
```

## Shell completions

```bash
reggae completion bash >> ~/.bashrc
reggae completion zsh  >> ~/.zshrc
reggae completion fish > ~/.config/fish/completions/reggae.fish
```

## Adding a provider

See [`src/extensions/README.md`](src/extensions/README.md).

## Development

```bash
cargo build
cargo test
cargo clippy
```
