/** Browser-only demo data for README screenshots (`?demo=1` / `?demo=empty`). */

export type DemoMode = "full" | "empty" | null;

export function detectDemoMode(): DemoMode {
  if (typeof window === "undefined") return null;
  const v = new URLSearchParams(window.location.search).get("demo");
  if (v === "1" || v === "full") return "full";
  if (v === "empty") return "empty";
  return null;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function demoProjects(): any[] {
  return [
    {
      id: "demo-storefront",
      name: "storefront",
      path: "/Users/demo/work/storefront",
      package_manager: "pnpm",
      pm_installed: true,
      pm_overridden: false,
      scripts: {
        dev: "vite",
        build: "tsc && vite build",
        lint: "eslint .",
      },
      exists: true,
      pinned: true,
      git_branch: "main",
      git_dirty: false,
      launch: {},
      autostart_scripts: [],
      env_files: [".env", ".env.local"],
    },
    {
      id: "demo-admin",
      name: "admin-dashboard",
      path: "/Users/demo/work/admin-dashboard",
      package_manager: "pnpm",
      pm_installed: true,
      pm_overridden: false,
      scripts: {
        dev: "next dev",
        test: "vitest",
      },
      exists: true,
      pinned: false,
      git_branch: "feat/ui",
      git_dirty: false,
      launch: {},
      autostart_scripts: [],
      env_files: [".env"],
    },
    {
      id: "demo-docs",
      name: "docs-site",
      path: "/Users/demo/work/docs-site",
      package_manager: "npm",
      pm_installed: true,
      pm_overridden: false,
      scripts: {
        dev: "vitepress dev",
        build: "vitepress build",
      },
      exists: true,
      pinned: false,
      git_branch: "main",
      git_dirty: true,
      launch: {},
      autostart_scripts: [],
      env_files: [],
    },
  ];
}

export function demoRunning(): Map<
  string,
  { key: string; pid: number; url: string | null; log_file: string }
> {
  return new Map([
    [
      "demo-storefront:dev",
      {
        key: "demo-storefront:dev",
        pid: 4242,
        url: "http://localhost:5173/",
        log_file: "/tmp/storefront-dev.log",
      },
    ],
    [
      "demo-admin:dev",
      {
        key: "demo-admin:dev",
        pid: 4243,
        url: "http://localhost:3000/",
        log_file: "/tmp/admin-dev.log",
      },
    ],
  ]);
}

export function demoLogs(): Map<string, string[]> {
  return new Map([
    [
      "demo-storefront:dev",
      [
        "$ pnpm dev",
        "VITE v7.0.4  ready in 312 ms",
        "",
        "  ➜  Local:   http://localhost:5173/",
        "  ➜  Network: http://192.168.1.24:5173/",
        "",
        "11:42:08 AM [vite] hmr update /src/App.tsx",
        "11:42:11 AM [vite] hmr update /src/components/Cart.tsx",
        "✓ compiled successfully",
        "11:43:02 AM [vite] page reload index.html",
      ],
    ],
    [
      "demo-admin:dev",
      [
        "$ pnpm dev",
        "▲ Next.js 15.1.0",
        "- Local:        http://localhost:3000",
        "✓ Ready in 1.2s",
      ],
    ],
  ]);
}
