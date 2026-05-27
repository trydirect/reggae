import type { Metadata } from "next";
import Link from "next/link";
import { contactChannels, contactTopics, site } from "@/lib/site";

export const metadata: Metadata = {
  title: "Contact us",
  description:
    "Reach the reggae maintainers for integration, support, and deployment conversations.",
};

export default function ContactPage() {
  return (
    <section className="mx-auto w-full max-w-6xl px-6 py-16 sm:px-8 lg:px-12 lg:py-24">
      <div className="max-w-3xl space-y-4">
        <p className="text-sm font-semibold uppercase tracking-[0.3em] text-[#2f7d32]">
          Contact us
        </p>
        <h1 className="text-4xl font-semibold tracking-tight text-[#141813] sm:text-5xl">
          Reach the team behind reggae.
        </h1>
        <p className="text-lg leading-8 text-[#4c5747]">
          The quickest way to talk to the project is through GitHub. Use the
          links below for bugs, feature requests, integration questions, or
          rollout planning around Stacker and domain automation.
        </p>
      </div>

      <div className="mt-12 grid gap-6 md:grid-cols-3">
        {contactChannels.map((channel) => (
          <a
            key={channel.title}
            className="rounded-3xl border border-[#141813]/10 bg-white/85 p-6 shadow-sm transition hover:-translate-y-0.5 hover:border-[#2f7d32]/30 hover:shadow-md"
            href={channel.href}
            target={channel.external ? "_blank" : undefined}
            rel={channel.external ? "noreferrer" : undefined}
          >
            <p className="text-sm font-semibold uppercase tracking-[0.25em] text-[#b03a2e]">
              {channel.label}
            </p>
            <h2 className="mt-4 text-2xl font-semibold text-[#141813]">
              {channel.title}
            </h2>
            <p className="mt-3 text-sm leading-7 text-[#4c5747]">
              {channel.description}
            </p>
          </a>
        ))}
      </div>

      <div className="mt-12 grid gap-8 lg:grid-cols-[1.1fr_0.9fr]">
        <article className="rounded-[2rem] border border-[#141813]/10 bg-[#141813] p-8 text-[#fffaf0] shadow-lg shadow-[#141813]/10">
          <p className="text-sm font-semibold uppercase tracking-[0.3em] text-[#f4d35e]">
            Helpful context
          </p>
          <h2 className="mt-4 text-3xl font-semibold tracking-tight">
            Include these details when you reach out.
          </h2>
          <ul className="mt-6 space-y-4 text-sm leading-7 text-[#f8ecd0]">
            {contactTopics.map((topic) => (
              <li
                key={topic}
                className="rounded-2xl border border-white/10 bg-white/5 px-4 py-3"
              >
                {topic}
              </li>
            ))}
          </ul>
        </article>

        <article className="rounded-[2rem] border border-[#141813]/10 bg-white/85 p-8 shadow-sm">
          <p className="text-sm font-semibold uppercase tracking-[0.3em] text-[#2f7d32]">
            Recommended rollout
          </p>
          <h2 className="mt-4 text-3xl font-semibold tracking-tight text-[#141813]">
            Start locally, then push to Hetzner.
          </h2>
          <p className="mt-4 text-sm leading-7 text-[#4c5747]">
            The website ships with a static-export Stacker config. Validate the
            stack locally first, then reuse the same config for Hetzner once
            your Stacker cloud login is active.
          </p>

          <div className="mt-6 rounded-3xl border border-[#141813]/10 bg-[#fffaf0] p-5">
            <p className="text-sm font-medium uppercase tracking-[0.25em] text-[#b03a2e]">
              Commands
            </p>
            <code className="mt-3 block overflow-x-auto text-sm leading-7 text-[#141813]">
              cd web{"\n"}stacker config validate{"\n"}stacker deploy --target
              local{"\n"}stacker deploy --target cloud --watch
            </code>
          </div>

          <div className="mt-6 flex flex-col gap-4 sm:flex-row">
            <a
              className="inline-flex items-center justify-center rounded-full bg-[#2f7d32] px-6 py-3 text-sm font-semibold text-[#fffaf0] transition hover:bg-[#256628]"
              href={site.issuesUrl}
              target="_blank"
              rel="noreferrer"
            >
              Open the issue tracker
            </a>
            <Link
              className="inline-flex items-center justify-center rounded-full border border-[#141813]/15 bg-white px-6 py-3 text-sm font-semibold text-[#141813] transition hover:border-[#2f7d32]/40"
              href="/"
            >
              Back to home
            </Link>
          </div>
        </article>
      </div>
    </section>
  );
}
