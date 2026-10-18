#import "/lib/lib.typ": *

#show: schema.with("page")

#let version = html.text(fill: rgb("#22D3EE"))[0.1.8]

#title[Migrating to Typsite #version]
#page-title[Migrating to Typsite 0.1.8]
#date[2026-10-18]
#author[Glomzzz]
#parent("index.typ")
#metacontent("summary")[Upgrade helpers, configure RSS, and migrate safely to the recoverable build engine in Typsite 0.1.8.]

Typsite #version replaces the build engine, fixes stale incremental output, and supports the latest stable Typst. Existing valid articles, templates, configuration keys, and CLI options remain supported. This guide covers the changes that need attention when upgrading from 0.1.7.

If you are upgrading from an earlier release, also read #cite-title("migrate-to-017.typ").

= Upgrade checklist

+ Back up your articles, `root/lib`, and `.typsite`, especially any custom templates or helpers.
+ Install Typsite *0.1.8* and the *latest stable Typst*. This release has been tested with Typst *0.15.1*. Older Typst releases are not a compatibility target.
+ Generate a separate reference project; do not initialize over your existing site:

  ```sh
  typsite init --dir ../typsite-018-reference
  ```

+ Compare that project's `root/lib` and `.typsite` with yours. Merge the updated helpers without overwriting your customizations.
+ Run your usual compile command. Old internal caches are automatically treated as cache misses; a manual clean is not normally necessary.

  ```sh
  typsite --version
  typst --version
  typsite compile
  ```

+ Check both the rendered site and the command's exit status before deploying. A failed build now exits with a nonzero status.

= Site helpers and Typst

== Update inline-math

Merge the updated `root/lib/site.typ` from the reference project. The `inline-math` helper now runs in a contextual evaluation, queries footnote labels, and passes them through to the SVG renderer. Both inline and block fallback are supported:

```typ
#inline-math($a/b$)
#inline-math($sum_(i=1)^n i$, block: true)
```

Your existing call sites do not need a new signature. Typsite passes the HTML feature flag and `html-frames` input to Typst automatically.

== Keep the bundled package import

The *Typsite executable* is version 0.1.8. The bundled *Typst package* keeps its existing identity:

```typ
#import "@local/typsite:0.1.0": *
```

Do not change this import to `@local/typsite:0.1.8`. The executable installs its bundled package before an export that needs it.

== Build the bundled demo

The initialized demo includes these migration pages, its templates, fonts, and SVG examples. It does not require preview-package downloads. From the reference project directory:

```sh
typsite compile
typsite compile --port 8080
```

The second command starts preview mode. Open `http://localhost:8080/` or `/en/`. External music players and web-font links in the content examples still need a browser network connection; they are not build dependencies.

= Cache and output ownership

The cache now stores raw Typst HTML and fingerprints of the files Typst actually read. Imports and included files invalidate the appropriate exports. Parsed relationships and rendered fragments are not restored from old caches.

When publication is dirty, Typsite rebuilds the current relationship graph and recomposes all pages. Only changed output bytes are written. Removing a citation removes its old backlink; deleting or renaming an article removes its old owned output.

The output directory contains a separate ownership journal:

```text
publish/.typsite-outputs.json
```

*Keep this file.* It is not a disposable cache. It lets Typsite remove stale generated files while preserving unrelated files. If it is corrupt, the build refuses to guess which files may be deleted. Back it up before repairing it; deleting it explicitly relinquishes ownership of previous outputs.

Outputs created before this journal existed are not automatically claimed or removed. If an old, deleted page remains after upgrading, remove that known obsolete file yourself, or explicitly clean the output directory after reviewing its contents.

== Safer clean

`typsite clean` validates both roots before deleting anything, takes the build locks, and refuses source/configuration overlap. Nonempty legacy directories without ownership markers require an explicit opt-in:

```sh
# Review these directories first: clean removes their contents.
typsite clean --cache ./.cache --output ./publish --force
```

If your source or configuration roots are nondefault, pass them too so they are protected:

```sh
typsite clean --input ./content --config ./config \
  --cache ./build-cache --output ./public --force
```

`--force` permits unmarked legacy directories; it does *not* bypass path-safety checks or active build locks. Lock files may remain after cleaning. Do not delete `.typsite.lock` while a build or preview process is running.

= Stricter validation

Previously ignored errors can now stop a build. Fix or remove invalid resources rather than relying on them being skipped:

- Schemas, components, and rewrite HTML must have complete `<head>...</head>` and `<body>...</body>` envelopes. A deliberately empty `<head/>` or `<body/>` is allowed.
- Malformed themes, syntax definitions, syntax metadata, and rewrite resources are errors, even if unused. Duplicate resource identities are rejected.
- A page cannot explicitly name itself as its parent or embed itself. Cycles in parent inheritance, embeds, or metadata dependencies are rejected instead of hanging. Ordinary mutual citations and self-citations remain allowed.
- Source, configuration, cache, and output roots must not overlap; package sources must not overlap cache or output roots. Output collisions, case-only collisions, escaping paths, and symlinked managed output paths are rejected. Spaces and bracket characters in directory names are supported literally.
- `.typsite.lock` and `.typsite-outputs.json` are reserved output names, including case variants and descendants beneath those names.

= Packages and reproducibility

`--packages` still names a directory of local package *sources*, not a Typst namespace. Invalid package names/versions and duplicate identities are rejected before installation.

For an isolated namespace, set `TYPST_PACKAGE_PATH`; packages will be installed beneath `local/<name>/<version>`. Typsite also honors `TYPST_PACKAGE_CACHE_PATH`. For example, in a POSIX shell:

```sh
export TYPST_PACKAGE_PATH="$PWD/.typst-packages"
export TYPST_PACKAGE_CACHE_PATH="$PWD/.typst-package-cache"
typsite compile
```

Keep these directories separate from the source, configuration, build cache, and output roots. Without an override, the default shared package installation behavior is retained. Use project fonts and a fixed `SOURCE_DATE_EPOCH` when you need reproducible exports; changes to system fonts or undeclared external inputs may require discarding the build cache.

= Parallel builds and --jobs

Independent file hashes, article parsing, and page rendering now use Rayon. Typst exports run concurrently with an automatic upper bound from the available CPU/Rayon budget (the earlier fixed four-article cap is removed). Each Typst subprocess receives a share of the CPU budget. Set `typsite compile --jobs 1` to serialize article exports on low-memory machines, or `--jobs 4` to set an explicit limit (`-j` is an alias; values must be positive).

`--jobs` does not control all Rayon work; use `RAYON_NUM_THREADS` to tune hashing/parsing/rendering. Packages are prepared before workers start, results are collected in stable source order, and all workers finish before checkpoint/publication. Graph/metadata ordering, stale deletions, and publication ownership remain protected. Independent output files publish in parallel with atomic renames and data syncing. Disposable caches now use checksummed binary manifests with shared dependency sets, plus a content-keyed highlighting-resource cache; old formats rebuild automatically. Verified no-ops still hash-check actual inputs/artifacts/outputs but no longer reparse all configuration. Installed local packages are fingerprinted before export, avoiding unnecessary verification passes; newly discovered external inputs still need verification. Changing concurrency alone needs no cache clean and produces the same output bytes.

= RSS feed configuration

RSS 2.0 supports summaries or full article content. Setup has *two separate parts*: enable feed generation in the site configuration, then choose articles through a marked collection. The article metadata `rss` *defaults to false*; setting the site URL alone does not select any articles.

== Step 1: Configure the site

Edit the existing `[site]` section in `.typsite/options.toml`; do not append a duplicate section. Replace the example URL with the public address where your site will be deployed:

```toml
[site]
base_url = "https://example.org"
title = "My site"
description = "Notes and articles"
rss_path = "/rss.xml"
rss_limit = 20
rss_content = "summary" # "summary" or "full"
sitemap_path = "/sitemap.xml"
```

Keep `rss = false` under `[default_metadata.options]` (or omit it). This is the per-article default, not a switch under `[site]`. Opt in on individual collection pages instead of setting every article to true.

== Step 2: Mark a collection and embed its entries

Merge the new `rss` import in `root/lib/lib.typ`, or import it directly from the bundled package as below. For example, create `root/feed.typ` using two existing demo articles:

```typ
#import "/lib/lib.typ": *
#import "@local/typsite:0.1.0": rss
#show: schema.with("page")
#title[Feed collection]
#rss(true)

= #embed("./typst.typ")
= #embed("./article.typ")
```

This selects `typst.typ` and `article.typ`, *not* `feed.typ`. In your own site, replace these paths with your articles. Only direct embeds are selected; nested embeds, citations, parent links, and graph-only child declarations do not add entries. Multiple containers contribute a deduplicated union. Marked containers are always excluded, even when embedded by another container; their marker is not inherited by their children. `#rss(false)` removes a container marker, not an item selected by another container.

== Step 3: Build and subscribe

```sh
typsite compile
typsite compile --port 8080
```

The first command writes `publish/rss.xml` (or your configured output/path). The second serves it at `http://localhost:8080/rss.xml` for inspection. Readers should subscribe to your deployed public URL, such as `https://example.org/rss.xml`, not the localhost preview address. Each generated HTML page includes an RSS autodiscovery link.

The demo's two homepages already use `#rss(true)`. With `base_url` set, their direct embeds produce four entries: the article guide and Typst introduction in both languages. The additional `feed.typ` above selects existing entries again, without duplicating them. Homepages, migrations, and example pages are not automatically included. The sitemap still includes all compiled articles.

== Content mode and common setup problems

- `rss_content = "summary"` is the default: use the first nonempty `description`, `summary`, or `abstract` metadata. Add `#metacontent("summary")[A short introduction.]` in the selected article; missing summaries do not exclude it.
- `rss_content = "full"` additionally includes composed article HTML without page navigation, sidebars, or reference/backlink footers. Embedded content remains within its selected article. Feed-reader support for SVG, MathML, and site-specific CSS varies.
- No `rss.xml`: check that `base_url` is nonempty, `rss_path` is nonempty, and `rss_limit` is greater than zero. The default path and limit remain `/rss.xml` and 20. An invalid nonempty site URL fails validation.
- An empty feed: check `#rss(true)` and actual direct `#embed(...)` calls. Setting only `[site].base_url` is not enough, and `rss = false` is not an item opt-out.
- RSS dates use RFC 2822, not ISO timestamps. Date-only and `YYYY-MM-DD HH:MM[:SS]` metadata use UTC; RFC 3339/RFC 2822 offsets are normalized to UTC. Invalid dates are omitted, and titles/summaries are extracted as text rather than exposing HTML tags.

More settings and article metadata examples are in #cite("index.typ")[the homepage's RSS section].

= Preview and failure recovery

Preview still uses a nonzero `--port`. Every connected client observes the same build revision. A true no-op does not reload browsers. Export, parse, or render failures keep the last successfully published pages; successful repair triggers a reload. Ctrl-C waits for the in-flight build before shutting down.

Successful exports can be reused after a later publication failure. Fix the reported error and rerun the same command. Publication is atomic *per file*, not for the whole site: a failed publication may leave a mixture of complete old and new files, which a normal rerun repairs. Use a separate deployment step if you need an all-or-nothing site switch.
