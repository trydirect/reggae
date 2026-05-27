import Link from "next/link";
import { featureHighlights, providerNames, site, workflowSteps } from "@/lib/site";

export default function HomePage() {
  return (
    <>
      <section className="mx-auto w-full max-w-6xl px-6 py-16 sm:px-8 lg:px-12 lg:py-24">
        <div className="grid gap-10 lg:grid-cols-[1.3fr_0.7fr] lg:items-center">
          <div className="space-y-8">
            <span className="inline-flex rounded-full border border-[#1f2415]/10 bg-white/80 px-4 py-2 text-sm font-medium tracking-wide text-[#2f7d32] shadow-sm">
              Open-source domain workflows for modern infrastructure teams
            </span>

            <div className="space-y-5">
              <h1 className="max-w-4xl text-4xl font-semibold tracking-tight text-[#141813] sm:text-5xl lg:text-6xl">
                Keep domain registrations, DNS changes, and Stacker rollouts in
                one steady rhythm.
              </h1>
              <p className="max-w-3xl text-lg leading-8 text-[#374131] sm:text-xl">
                {site.description} Ship registrar operations with a single CLI,
                then carry the same workflow into your deployment pipeline.
              </p>
            </div>

            <div className="flex flex-col gap-4 sm:flex-row">
              <a
                className="inline-flex items-center justify-center rounded-full bg-[#2f7d32] px-6 py-3 text-sm font-semibold text-[#fffaf0] transition hover:bg-[#256628]"
                href={site.repoUrl}
                target="_blank"
                rel="noreferrer"
              >
                Explore the repository
              </a>
              <Link
                className="inline-flex items-center justify-center rounded-full border border-[#141813]/15 bg-white/80 px-6 py-3 text-sm font-semibold text-[#141813] transition hover:border-[#2f7d32]/40 hover:bg-white"
                href="/contact"
              >
                Contact us
              </Link>
            </div>

            <div className="rounded-3xl border border-[#141813]/10 bg-[#141813] px-6 py-5 text-[#fffaf0] shadow-lg shadow-[#141813]/10">
              <p className="text-sm font-medium uppercase tracking-[0.3em] text-[#f4d35e]">
                Install
              </p>
              <code className="mt-3 block overflow-x-auto text-sm leading-7 text-[#f8ecd0]">
                {site.installCommand}
              </code>
            </div>
          </div>

          <div className="grid gap-4">
            <article className="rounded-3xl border border-[#141813]/10 bg-white/85 p-6 shadow-sm">
              <p className="text-sm font-medium uppercase tracking-[0.3em] text-[#b03a2e]">
                Built for operators
              </p>
              <p className="mt-4 text-3xl font-semibold text-[#141813]">
                4 providers
              </p>
              <p className="mt-2 text-sm leading-7 text-[#4c5747]">
                Porkbun, Cloudflare, GoDaddy, and Namecheap supported through a
                single command surface.
              </p>
            </article>

            <article className="rounded-3xl border border-[#141813]/10 bg-[#fffaf0]/90 p-6 shadow-sm">
              <p className="text-sm font-medium uppercase tracking-[0.3em] text-[#2f7d32]">
                Built for automation
              </p>
              <p className="mt-4 text-3xl font-semibold text-[#141813]">
                JSON-ready
              </p>
              <p className="mt-2 text-sm leading-7 text-[#4c5747]">
                Structured output makes it easy to pipe reggae into Stacker and
                wider delivery workflows.
              </p>
            </article>

            <article className="rounded-3xl border border-[#141813]/10 bg-white/85 p-6 shadow-sm">
              <p className="text-sm font-medium uppercase tracking-[0.3em] text-[#f4d35e]">
                Built for uptime
              </p>
              <p className="mt-4 text-3xl font-semibold text-[#141813]">
                Expiry alerts
              </p>
              <p className="mt-2 text-sm leading-7 text-[#4c5747]">
                Built-in scheduling helps teams catch expiring domains before
                they become production incidents.
              </p>
            </article>
          </div>
        </div>
      </section>

      <section className="mx-auto w-full max-w-6xl px-6 py-10 sm:px-8 lg:px-12">
        <div className="rounded-[2rem] border border-[#141813]/10 bg-white/80 p-8 shadow-sm">
          <div className="max-w-2xl space-y-3">
            <p className="text-sm font-semibold uppercase tracking-[0.3em] text-[#2f7d32]">
              Why reggae
            </p>
            <h2 className="text-3xl font-semibold tracking-tight text-[#141813]">
              Designed for domain operations without the registrar lock-in.
            </h2>
            <p className="text-base leading-8 text-[#4c5747]">
              From quick availability checks to DNS automation, reggae keeps the
              hard parts of domain management consistent across providers.
            </p>
          </div>

          <div className="mt-8 grid gap-5 md:grid-cols-2">
            {featureHighlights.map((feature) => (
              <article
                key={feature.title}
                className="rounded-3xl border border-[#141813]/10 bg-[#fffaf0] p-6"
              >
                <p className="text-sm font-semibold uppercase tracking-[0.25em] text-[#b03a2e]">
                  {feature.label}
                </p>
                <h3 className="mt-4 text-2xl font-semibold text-[#141813]">
                  {feature.title}
                </h3>
                <p className="mt-3 text-sm leading-7 text-[#4c5747]">
                  {feature.description}
                </p>
              </article>
            ))}
          </div>
        </div>
      </section>

      <section className="mx-auto w-full max-w-6xl px-6 py-10 sm:px-8 lg:px-12">
        <div className="grid gap-8 lg:grid-cols-[0.9fr_1.1fr]">
          <div className="space-y-3">
            <p className="text-sm font-semibold uppercase tracking-[0.3em] text-[#2f7d32]">
              Workflow
            </p>
            <h2 className="text-3xl font-semibold tracking-tight text-[#141813]">
              A calm path from domain lookup to deployment.
            </h2>
            <p className="text-base leading-8 text-[#4c5747]">
              Keep the operational flow readable for both humans and automation.
            </p>
          </div>

          <div className="grid gap-4">
            {workflowSteps.map((step, index) => (
              <article
                key={step.title}
                className="rounded-3xl border border-[#141813]/10 bg-white/85 p-6 shadow-sm"
              >
                <div className="flex items-center gap-4">
                  <span className="flex h-10 w-10 items-center justify-center rounded-full bg-[#141813] text-sm font-semibold text-[#fffaf0]">
                    0{index + 1}
                  </span>
                  <h3 className="text-xl font-semibold text-[#141813]">
                    {step.title}
                  </h3>
                </div>
                <p className="mt-4 text-sm leading-7 text-[#4c5747]">
                  {step.description}
                </p>
              </article>
            ))}
          </div>
        </div>
      </section>

      <section className="mx-auto w-full max-w-6xl px-6 py-10 pb-20 sm:px-8 lg:px-12 lg:pb-24">
        <div className="rounded-[2rem] border border-[#141813]/10 bg-[#141813] p-8 text-[#fffaf0] shadow-lg shadow-[#141813]/10">
          <div className="max-w-2xl space-y-3">
            <p className="text-sm font-semibold uppercase tracking-[0.3em] text-[#f4d35e]">
              Supported providers
            </p>
            <h2 className="text-3xl font-semibold tracking-tight">
              One interface, multiple registrars.
            </h2>
            <p className="text-base leading-8 text-[#f8ecd0]">
              Start with a single provider or standardize your whole portfolio
              from one CLI.
            </p>
          </div>

          <div className="mt-8 flex flex-wrap gap-3">
            {providerNames.map((provider) => (
              <span
                key={provider}
                className="rounded-full border border-white/15 bg-white/10 px-4 py-2 text-sm font-medium"
              >
                {provider}
              </span>
            ))}
          </div>
        </div>
      </section>
    </>
  );
}
