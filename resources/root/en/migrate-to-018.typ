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

= RSS feed configuration

RSS remains opt-in: set `[site].base_url` to your public HTTP(S) URL. The default `rss_path = "/rss.xml"` and `rss_limit = 20` are unchanged. New `rss_content = "summary"` retains summaries and links; choose `"full"` to include composed article HTML as well. Every generated page receives an RSS autodiscovery link when enabled. See #cite("index.typ")[the RSS configuration section on the homepage] for a complete example.

RSS no longer includes every compiled article. Add `#rss(true)` to a collection page: only its direct embeds become items, and marked containers themselves are always excluded. Multiple containers are deduplicated; the boolean defaults to false and is not inherited. Nested embeds, citations, or parent relationships do not select separate items. Without a marked container, the enabled feed is empty. Import `rss` from `@local/typsite:0.1.0`, or merge the import list from the new `root/lib/lib.typ`. `#rss(false)` removes the container marker; it is not an item opt-out. The sitemap remains independent and includes all compiled pages.

RSS dates now use RFC 2822 rather than ISO timestamps. Date-only and `YYYY-MM-DD HH:MM[:SS]` metadata are treated as UTC; RFC 3339/RFC 2822 offsets are normalized to UTC. Invalid calendar dates are omitted. Titles and summaries are extracted as text from Typst metadata instead of exposing literal HTML tags. Invalid nonempty site URLs now fail configuration validation.

= Preview and failure recovery

Preview still uses a nonzero `--port`. Every connected client observes the same build revision. A true no-op does not reload browsers. Export, parse, or render failures keep the last successfully published pages; successful repair triggers a reload. Ctrl-C waits for the in-flight build before shutting down.

Successful exports can be reused after a later publication failure. Fix the reported error and rerun the same command. Publication is atomic *per file*, not for the whole site: a failed publication may leave a mixture of complete old and new files, which a normal rerun repairs. Use a separate deployment step if you need an all-or-nothing site switch.
