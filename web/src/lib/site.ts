export const site = {
  name: "reggae",
  description:
    "A cross-registrar CLI that keeps domain lookup, registration, DNS changes, and expiry monitoring under one clear interface.",
  repoUrl: "https://github.com/trydirect/reggae",
  issuesUrl: "https://github.com/trydirect/reggae/issues",
  installCommand:
    "curl -sSL https://raw.githubusercontent.com/trydirect/reggae/main/installer.sh | bash",
} as const;

export const providerNames = [
  "Porkbun",
  "Cloudflare",
  "GoDaddy",
  "Namecheap",
] as const;

export const featureHighlights = [
  {
    label: "Availability",
    title: "Search and price domains quickly",
    description:
      "Check availability and compare pricing before you commit, without switching registrar dashboards.",
  },
  {
    label: "Provisioning",
    title: "Register domains with consistent commands",
    description:
      "Unify the registration flow across providers so your team learns one interface instead of four.",
  },
  {
    label: "DNS",
    title: "Manage records from one CLI",
    description:
      "List, add, and update A, AAAA, CNAME, MX, TXT, NS, and other records without provider-specific syntax.",
  },
  {
    label: "Monitoring",
    title: "Track renewals before they surprise you",
    description:
      "Run scheduled expiry checks and feed the results into webhook-based alerting or broader ops workflows.",
  },
] as const;

export const workflowSteps = [
  {
    title: "Check the domain",
    description:
      "Query availability, inspect pricing, and decide which registrar or provider fits the rollout.",
  },
  {
    title: "Provision and configure",
    description:
      "Register the domain, attach DNS records, and keep the exact sequence scriptable for future environments.",
  },
  {
    title: "Automate the follow-through",
    description:
      "Use JSON output and expiry notifications to fold domain operations into Stacker and your delivery pipeline.",
  },
] as const;

export const contactChannels = [
  {
    label: "Repository",
    title: "Review the source",
    description:
      "Browse the CLI, examples, and deployment assets directly in the public repository.",
    href: site.repoUrl,
    external: true,
  },
  {
    label: "Support",
    title: "Open an issue",
    description:
      "Report bugs, request features, or describe the integration path you want the maintainers to help with.",
    href: site.issuesUrl,
    external: true,
  },
  {
    label: "Install",
    title: "Try the CLI first",
    description:
      "Validate the workflow locally, capture your target registrar or deployment needs, and bring that context into the conversation.",
    href: `${site.repoUrl}#quick-start`,
    external: true,
  },
] as const;

export const contactTopics = [
  "What provider or registrar you want to automate first.",
  "Whether you are integrating reggae into Stacker or using it as a standalone CLI.",
  "The domain lifecycle steps you want covered, such as pricing, registration, DNS, or renewal checks.",
  "Any logs, command output, or deployment details that can speed up troubleshooting.",
] as const;
