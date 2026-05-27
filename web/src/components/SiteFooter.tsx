import { site } from "@/lib/site";

export default function SiteFooter() {
  return (
    <footer className="border-t border-[#141813]/10 bg-white/50">
      <div className="mx-auto flex w-full max-w-6xl flex-col gap-4 px-6 py-8 text-sm text-[#4c5747] sm:px-8 lg:flex-row lg:items-center lg:justify-between lg:px-12">
        <p>
          <span className="font-semibold text-[#141813]">{site.name}</span> is
          an open-source CLI for cross-registrar domain and DNS operations.
        </p>
        <a
          className="font-medium text-[#2f7d32] transition hover:text-[#256628]"
          href={site.repoUrl}
          target="_blank"
          rel="noreferrer"
        >
          View the project on GitHub
        </a>
      </div>
    </footer>
  );
}
