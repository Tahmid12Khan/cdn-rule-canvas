import type { Metadata } from "next";
import { Inter, JetBrains_Mono } from "next/font/google";

import "./globals.css";
import { Breadcrumbs } from "@/components/nav/Breadcrumbs";
import { TopNav } from "@/components/nav/TopNav";
import { OnboardingChecklist } from "@/components/onboarding/OnboardingChecklist";
import { QueryProvider } from "@/components/providers/QueryProvider";

const inter = Inter({ subsets: ["latin"], variable: "--font-inter" });
const jetBrainsMono = JetBrains_Mono({
  subsets: ["latin"],
  variable: "--font-mono",
});

export const metadata: Metadata = {
  title: "Response Rule Engine",
  description:
    "Author, version, and deploy response transformation rules with a visual rule builder.",
};

// Pre-paint theme bootstrap. Runs synchronously in <head> BEFORE first paint so
// the correct `.light` / `.dark` class is on <html> before React hydrates — no
// flash of the wrong theme, no hydration mismatch. Reads the SAME localStorage
// shape that zustand persist writes ({state:{theme:"..."},version:n}); precedence
// is stored theme > prefers-color-scheme > 'dark'.
const themeBootstrap = `(function(){try{var t;var s=localStorage.getItem("rre-theme-v1");if(s){t=JSON.parse(s).state.theme;}if(t!=="light"&&t!=="dark"){t=window.matchMedia&&window.matchMedia("(prefers-color-scheme: dark)").matches?"dark":(window.matchMedia?"light":"dark");}var r=document.documentElement;r.classList.add(t);r.classList.remove(t==="dark"?"light":"dark");}catch(e){document.documentElement.classList.add("dark");}})();`;

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html
      lang="en"
      suppressHydrationWarning
      className={`${inter.variable} ${jetBrainsMono.variable}`}
    >
      <head>
        <script dangerouslySetInnerHTML={{ __html: themeBootstrap }} />
      </head>
      <body className="min-h-screen bg-bg font-sans text-fg antialiased">
        <QueryProvider>
          <TopNav />
          <Breadcrumbs />
          <main className="mx-auto w-full max-w-6xl px-6 py-8">{children}</main>
          <OnboardingChecklist />
        </QueryProvider>
      </body>
    </html>
  );
}
