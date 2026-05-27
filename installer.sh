#!/usr/bin/env bash
set -euo pipefail

REPO="trydirect/reggae"
BINARY="reggae"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

detect_platform() {
    local os arch
    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)

    case "$os" in
        linux) os="unknown-linux-gnu" ;;
        darwin) os="apple-darwin" ;;
        *) echo "Unsupported OS: $os" >&2; exit 1 ;;
    esac

    case "$arch" in
        x86_64 | amd64) arch="x86_64" ;;
        aarch64 | arm64) arch="aarch64" ;;
        *) echo "Unsupported arch: $arch" >&2; exit 1 ;;
    esac

    echo "${arch}-${os}"
}

latest_version() {
    curl -sSf "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | sed 's/.*"tag_name": "\(.*\)".*/\1/'
}

write_default_config() {
    local config_dir="${HOME}/.reggae"
    local config_file="${config_dir}/config.yaml"

    if [[ -f "$config_file" ]]; then
        echo "  Config already exists — skipping: ${config_file}"
        return
    fi

    mkdir -p "$config_dir"
    chmod 700 "$config_dir"

    # Write via a temp file then rename — avoids a partial file on interrupt
    local tmp_file
    tmp_file=$(mktemp "${config_dir}/.config.yaml.XXXXXX")

    cat > "$tmp_file" << 'EOF'
# reggae configuration
# Uncomment the provider(s) you use and fill in your credentials.
# File permissions should stay 600 (owner read/write only).
#
# Priority order for credentials:
#   1. Environment variables (highest)
#   2. This file
#
# Full list of supported env vars: https://github.com/trydirect/reggae#configuration

# Which provider to use when --provider is not specified on the command line.
# Supported: porkbun | cloudflare | godaddy | namecheap
default_provider: porkbun

providers:

  # ── Porkbun ──────────────────────────────────────────────────────────────
  # Env vars: DM_PROVIDERS_PORKBUN_API_KEY, DM_PROVIDERS_PORKBUN_SECRET_API_KEY
  # Get keys at: https://porkbun.com/account/api
  porkbun:
    api_key: ""          # pk1_...
    secret_api_key: ""   # sk1_...

  # ── Cloudflare ───────────────────────────────────────────────────────────
  # Env vars: DM_PROVIDERS_CLOUDFLARE_API_TOKEN, DM_PROVIDERS_CLOUDFLARE_ACCOUNT_ID
  # Create a token at: https://dash.cloudflare.com/profile/api-tokens
  # account_id is required for Registrar API (domain purchase/transfer).
  # cloudflare:
  #   api_token: ""      # Bearer token — needs Zone:Edit + Account:Registrar:Edit
  #   account_id: ""     # Found in the right sidebar of your Cloudflare dashboard

  # ── GoDaddy ──────────────────────────────────────────────────────────────
  # Env vars: DM_PROVIDERS_GODADDY_API_KEY, DM_PROVIDERS_GODADDY_API_SECRET
  # Get keys at: https://developer.godaddy.com/keys
  # godaddy:
  #   api_key: ""
  #   api_secret: ""
  #   sandbox: false     # Set true to use OTE (test) environment
  #   consent_ip: ""     # Your public IP — required for domain purchase consent

  # ── Namecheap ────────────────────────────────────────────────────────────
  # Env vars: DM_PROVIDERS_NAMECHEAP_API_USER, DM_PROVIDERS_NAMECHEAP_API_KEY
  # Enable API access at: https://ap.www.namecheap.com/settings/tools/apiaccess/
  # Your IP must be whitelisted in the Namecheap dashboard.
  # namecheap:
  #   api_user: ""       # Your Namecheap username
  #   api_key: ""
  #   username: ""       # Usually same as api_user
  #   client_ip: ""      # Your whitelisted public IP
  #   sandbox: false     # Set true to use sandbox.namecheap.com

# ── Default registrant contact ───────────────────────────────────────────────
# Used when registering a domain (GoDaddy, Namecheap, Porkbun).
# registrant:
#   first_name: ""
#   last_name: ""
#   email: ""
#   phone: "+1.5555555555"   # E.164 format with leading +countrycode.
#   address1: ""
#   city: ""
#   state: ""                # 2-letter state/province code, e.g. CA
#   country: ""              # 2-letter ISO country code, e.g. US
#   postal_code: ""

# ── Expiry monitoring ────────────────────────────────────────────────────────
scheduler:
  enabled: false
  check_interval_secs: 86400   # How often to check (seconds). Default: daily.
  expiry_lead_days: 30         # Warn this many days before expiry.
  domains: []                  # Domains to monitor, e.g. [example.com, mysite.io]
  notification:
    type: stdout               # stdout | webhook
    # webhook_url: https://hooks.example.com/notify

# ── Logging ──────────────────────────────────────────────────────────────────
logging:
  level: info                  # trace | debug | info | warn | error
EOF

    chmod 600 "$tmp_file"
    mv "$tmp_file" "$config_file"

    echo "  ✓ Default config written to ${config_file} (permissions: 600)"
    echo "  → Open it and uncomment the provider(s) you want to use."
}

main() {
    local platform version download_url tmp_dir

    platform=$(detect_platform)
    version=$(latest_version)

    echo "Installing reggae ${version} for ${platform}…"

    download_url="https://github.com/${REPO}/releases/download/${version}/${BINARY}-${platform}.tar.gz"
    tmp_dir=$(mktemp -d)
    trap "rm -rf ${tmp_dir}" EXIT

    curl -sSfL "$download_url" | tar -xz -C "$tmp_dir"

    if [[ -w "$INSTALL_DIR" ]]; then
        install -m755 "${tmp_dir}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
    else
        echo "Installing to ${INSTALL_DIR} requires sudo…"
        sudo install -m755 "${tmp_dir}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
    fi

    echo "✓ reggae installed to ${INSTALL_DIR}/${BINARY}"

    write_default_config

    echo ""
    echo "  Run: reggae --help"
}

main "$@"
