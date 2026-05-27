import Link from "next/link";

const navigation = [
  { href: "/", label: "Home" },
  { href: "/contact", label: "Contact us" },
] as const;

export default function SiteHeader() {
  return (
    <header className="sticky top-0 z-10 border-b border-[#141813]/10 bg-[#fffaf0]/85 backdrop-blur">
      <div className="mx-auto flex w-full max-w-6xl items-center justify-between px-6 py-4 sm:px-8 lg:px-12">
        <Link href="/" className="flex items-center gap-3">
          <span className="flex h-11 w-11 items-center justify-center rounded-full bg-[#141813] text-base font-semibold uppercase tracking-[0.2em] text-[#fffaf0]">
            rg
          </span>
          <div>
            <p className="text-lg font-semibold text-[#141813]">reggae</p>
            <p className="text-xs uppercase tracking-[0.25em] text-[#4c5747]">
              Domain workflow CLI
            </p>
          </div>
        </Link>

        <nav aria-label="Primary">
          <ul className="flex items-center gap-2 rounded-full border border-[#141813]/10 bg-white/70 p-1 text-sm font-medium text-[#141813] shadow-sm">
            {navigation.map((item) => (
              <li key={item.href}>
                <Link
                  href={item.href}
                  className="inline-flex rounded-full px-4 py-2 transition hover:bg-[#2f7d32] hover:text-[#fffaf0]"
                >
                  {item.label}
                </Link>
              </li>
            ))}
          </ul>
        </nav>
      </div>
    </header>
  );
}
