# Research: Astro on Cloudflare Pages, /install redirect, Web Analytics

Resolves [#34](https://github.com/frankieramirez/bushel/issues/34), part of map [#31](https://github.com/frankieramirez/bushel/issues/31). Verified against official docs fetched 2026-08-25.

## 1. Deploying Astro to Cloudflare: static output, no adapter

- A static Astro site does **not** need the `@astrojs/cloudflare` adapter. Astro's deploy guide: the adapter is only needed "If your site uses on-demand rendering." Cloudflare's Pages framework guide agrees — the adapter exists for SSR sites that "render on Pages Functions."
- Deployment methods: Cloudflare Pages git integration works with build command `npm run build`, output directory `dist` (Cloudflare Pages framework guide for Astro). Wrangler CLI deploy also works.
- Caveat worth knowing: Cloudflare's Pages docs now carry an advisory — "Workers supports most Pages use cases and offers a broader feature set. It is Cloudflare's primary platform for building applications. Start new projects with Workers." Astro's own deploy guide has likewise shifted to Workers-first (static assets via `wrangler.jsonc` with `"directory": "./dist"`, still adapter-free for static). Pages remains fully functional; for a simple static site either works, but Workers (with static assets) is the forward-looking choice.

Sources:
- https://docs.astro.build/en/guides/deploy/cloudflare/
- https://developers.cloudflare.com/pages/framework-guides/deploy-an-astro-site/
- https://developers.cloudflare.com/pages/

## 2. `/install` 302 redirect via `_redirects`

- Put a `_redirects` file in Astro's `public/` directory (it lands in the build output root; Pages reads it from there and never serves it as an asset).
- Format: `[source] [destination] [code?]`, one rule per line. `302` is the **default** status code; 301/302/303/307/308 supported. External absolute URLs are valid redirect destinations (only 200-proxying is restricted to same-site relative URLs).
- Rule for bushel:

  ```
  /install https://github.com/frankieramirez/bushel/releases/latest/download/bushel-installer.sh 302
  ```

- Limits (far above our needs): 2,000 static + 100 dynamic rules, 1,000 chars per rule.
- curl chain verified live today: `https://github.com/frankieramirez/bushel/releases/latest/download/bushel-installer.sh` issues 2 redirects (latest-release resolver → signed `release-assets.githubusercontent.com` URL) ending in 200. Adding the Pages hop makes 3; `curl -L` follows cross-host redirects by default (default `--max-redirs` is 50), so `curl -LsSf https://bushel.sh/install | sh` survives the full chain. The v0.3.1 release does ship a `bushel-installer.sh` asset.

Sources:
- https://developers.cloudflare.com/pages/configuration/redirects/
- Live check: `curl -sIL` against the GitHub URL (2 redirects, final 200)

## 3. Cloudflare Web Analytics on Pages

- One-click automatic injection: Pages project → **Metrics** → **Enable** under Web Analytics. "Cloudflare will automatically add the JavaScript snippet to your Pages site on the next deployment." No manual snippet needed.
- Only caveat: pages must be valid HTML for the injection to happen. No custom-domain or proxy restrictions are documented for Pages sites.

Source: https://developers.cloudflare.com/pages/how-to/web-analytics/

## 4. Custom domain with DNS at Namecheap

- **Apex domain (`bushel.sh`) requires moving nameservers to Cloudflare.** Pages docs: "To use a custom apex domain … configure your nameservers to point to Cloudflare's nameservers." A CNAME at Namecheap only works for subdomains (`www.bushel.sh` → `<project>.pages.dev`), not the apex.
- Order matters: add the custom domain in the Pages dashboard **first**, then change DNS, or resolution errors occur.
- Manual Namecheap steps (per Namecheap KB):
  1. Add the site/zone in Cloudflare and note the two assigned nameservers.
  2. Namecheap → Domain List → **Manage** next to the domain.
  3. Nameservers section → select **Custom DNS**.
  4. Enter Cloudflare's two nameservers (name only, no IPs) and save via the green checkmark.
  5. Wait for propagation — Namecheap says up to 24 hours (rarely more). Existing host records do not carry over automatically; recreate any needed records in Cloudflare DNS.

Sources:
- https://developers.cloudflare.com/pages/configuration/custom-domains/
- https://www.namecheap.com/support/knowledgebase/article.aspx/767/10/how-to-change-dns-for-a-domain/
