# kyfetch

Simple internal-URL crawler — a mini [Screaming Frog](https://www.screamingfrog.co.uk/seo-spider/). Fetches **internal (same-domain) URLs only**. Async, fast, single binary.

## Install

### Homebrew

```sh
brew install KyzoonBD/tap/kyfetch
```

### From source

```sh
git clone https://github.com/KyzoonBD/kyfetch.git
cd kyfetch
cargo install --path .
```

## Usage

### Interactive

Run with no URL — kyfetch prompts for everything:

```sh
kyfetch
```

It asks for: site URL, how many URLs (a number or `all`), concurrency,
interval, shows a live progress spinner, then asks how to export.

### Flags (scripting)

```sh
kyfetch https://example.com
kyfetch example.com -n 1000 -c 50 -o urls.txt
kyfetch example.com -x report.xlsx
kyfetch example.com -i 200          # 200ms between requests
```

| Flag | Meaning | Default |
|------|---------|---------|
| `-n, --max-pages` | max pages to crawl (0 = all) | 500 |
| `-c, --concurrency` | parallel requests | 20 |
| `-t, --timeout` | request timeout (sec) | 10 |
| `-i, --interval` | delay between requests (ms) — rate-limit | 0 |
| `-o, --output` | save URLs to text file | — |
| `-x, --xlsx` | export results to `.xlsx` | — |

Output: `status  url  [content-type]`, one row per page. Errors show `ERR [reason]`.

## What it does

- Async BFS crawl, same-domain links only
- Real HTML parse → resolves relative URLs (`/foo`, `../bar`)
- Strips fragment + trailing slash → clean dedupe
- Follows only `text/html` for more links

---

## Maintainer: publish a Homebrew release

1. **Push code + tag a release:**

   ```sh
   git add -A && git commit -m "release v0.1.0"
   git tag v0.1.0
   git push origin main --tags
   ```

2. **Get the tarball sha256:**

   ```sh
   curl -sL https://github.com/KyzoonBD/kyfetch/archive/refs/tags/v0.1.0.tar.gz | shasum -a 256
   ```

3. **Create a tap repo** named `homebrew-tap` under your account
   (`https://github.com/KyzoonBD/homebrew-tap`), then copy the formula:

   ```sh
   # in the homebrew-tap repo
   mkdir -p Formula
   cp /path/to/kyfetch/Formula/kyfetch.rb Formula/
   # paste the sha256 from step 2 into kyfetch.rb
   git add -A && git commit -m "kyfetch 0.1.0" && git push
   ```

4. **Users install:**

   ```sh
   brew install KyzoonBD/tap/kyfetch
   ```

   `KyzoonBD/tap` = the `homebrew-tap` repo (Homebrew strips the `homebrew-` prefix).

### Test the formula locally before publishing

```sh
brew install --build-from-source ./Formula/kyfetch.rb
```
