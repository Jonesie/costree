window.BENCHMARK_DATA = {
  "lastUpdate": 1786392384231,
  "repoUrl": "https://github.com/Jonesie/costree",
  "entries": {
    "Benchmark": [
      {
        "commit": {
          "author": {
            "email": "peter@jonesie.net.nz",
            "name": "Jonesie",
            "username": "Jonesie"
          },
          "committer": {
            "email": "peter@jonesie.net.nz",
            "name": "Jonesie",
            "username": "Jonesie"
          },
          "distinct": true,
          "id": "094926cf35aa49c43d15feec3bde4fd8bfb2bfc8",
          "message": "CI: bump dorny/test-reporter and upload-artifact off Node 20\n\nBoth were pinned to majors (test-reporter@v1, upload-artifact@v4)\nwhose action.yml still declares node20, which GitHub now force-runs\nunder node24 with a deprecation warning. Bumped to the latest majors\n(v3 and v7 respectively, both declaring node24 natively) — checked\nboth changelogs/action.yml inputs first; the name/path/reporter/\nfail-on-error inputs we use are unchanged across those version jumps.\n\nrelease.yml's softprops/action-gh-release@v3 was already node24, no\nchange needed there.",
          "timestamp": "2026-08-11T02:46:57+12:00",
          "tree_id": "64bdf3d2af4a30dc44a79e6b1d4ae94e820e80f8",
          "url": "https://github.com/Jonesie/costree/commit/094926cf35aa49c43d15feec3bde4fd8bfb2bfc8"
        },
        "date": 1786373349852,
        "tool": "cargo",
        "benches": [
          {
            "name": "scan_synthetic_tree",
            "value": 1512727,
            "range": "± 59507",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "peter@jonesie.net.nz",
            "name": "Jonesie",
            "username": "Jonesie"
          },
          "committer": {
            "email": "peter@jonesie.net.nz",
            "name": "Jonesie",
            "username": "Jonesie"
          },
          "distinct": true,
          "id": "079ab65d7e1df1ebf99d570293739bacf99c36fb",
          "message": "README: add a Performance section with the benchmark dashboard link\n\nDocuments the rayon-parallel scanning, how to run the criterion\nbenchmark locally, and links to the CI-tracked benchmark history\ndashboard (dev/bench/, not the Pages root, which has no index.html).",
          "timestamp": "2026-08-11T03:00:31+12:00",
          "tree_id": "4a97cb57fd40842eb3349f9d85fcc548bd5af9c1",
          "url": "https://github.com/Jonesie/costree/commit/079ab65d7e1df1ebf99d570293739bacf99c36fb"
        },
        "date": 1786374177355,
        "tool": "cargo",
        "benches": [
          {
            "name": "scan_synthetic_tree",
            "value": 1186830,
            "range": "± 47229",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "peter@jonesie.net.nz",
            "name": "Jonesie",
            "username": "Jonesie"
          },
          "committer": {
            "email": "peter@jonesie.net.nz",
            "name": "Jonesie",
            "username": "Jonesie"
          },
          "distinct": true,
          "id": "785c321957d187e751f1081f465ddac1221190e1",
          "message": "Add Buy Me A Coffee link to README (#17)\n\nSame format used in the Michael repo: an Author section with the\nbuymeacoffee.com/jonesie link and badge image, placed before License.",
          "timestamp": "2026-08-11T03:14:00+12:00",
          "tree_id": "c72b2cdbdfc06fd8af966954c1f9bb10132abfdc",
          "url": "https://github.com/Jonesie/costree/commit/785c321957d187e751f1081f465ddac1221190e1"
        },
        "date": 1786374971980,
        "tool": "cargo",
        "benches": [
          {
            "name": "scan_synthetic_tree",
            "value": 1449858,
            "range": "± 51888",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "peter@jonesie.net.nz",
            "name": "Jonesie",
            "username": "Jonesie"
          },
          "committer": {
            "email": "peter@jonesie.net.nz",
            "name": "Jonesie",
            "username": "Jonesie"
          },
          "distinct": true,
          "id": "b98661d9c65b9e0c0f4e6f3e4157e0d98f56c385",
          "message": "Parallelize search matching, cap ancestor-walk cost, add benchmarks (#1)\n\nAnswers the issue's open questions with real numbers from a new\nbenches/search_benchmark.rs against a synthetic 500k-entry index:\n\n- Matching (regex.is_match per entry) now runs over rayon's shared\n  pool instead of sequentially — 24.3ms -> 4.4ms for a narrow query,\n  the common case while typing.\n- The real bottleneck for a broad query (e.g. a single common letter,\n  matching a large fraction of the tree) wasn't matching at all — it\n  was search_index() doing the full ancestor-walk to build the entire\n  match set, even though the view only ever renders\n  MAX_SEARCH_RESULTS (1000) rows. That constant moves from view.rs to\n  scanner.rs and now bounds search_index() itself: once the result\n  reaches it, the ancestor-walk stops. Broad-query cost: 293ms\n  uncapped -> 5.4ms capped, matching what the app actually runs.\n- Also tightened the ancestor-walk to check `contains()` before\n  allocating an owned PathBuf to insert, avoiding a wasted allocation\n  on every already-recorded ancestor — measurably neutral on its own\n  in benchmarks, but free and correctness-preserving, so kept.\n\nThe existing \"Listing directory…\" braille spinner now also drives a\n\"Searching…\" indicator (the Tick subscription needed extending to run\nwhile app.searching too, not just while listing/scanning) — visible\nprogress replacing the previous static text, per request.",
          "timestamp": "2026-08-11T07:29:58+12:00",
          "tree_id": "3212c46db3d238ecd7666ebb333592e06d4a885d",
          "url": "https://github.com/Jonesie/costree/commit/b98661d9c65b9e0c0f4e6f3e4157e0d98f56c385"
        },
        "date": 1786390334593,
        "tool": "cargo",
        "benches": [
          {
            "name": "scan_synthetic_tree",
            "value": 1004943,
            "range": "± 63585",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "peter@jonesie.net.nz",
            "name": "Jonesie",
            "username": "Jonesie"
          },
          "committer": {
            "email": "peter@jonesie.net.nz",
            "name": "Jonesie",
            "username": "Jonesie"
          },
          "distinct": true,
          "id": "df72f9f10109b4acbc0795e50b4ff3a899d5274e",
          "message": "CI: also run the new search_benchmark\n\nThe \"Run benchmark\" step only invoked scan_benchmark; the new\nsearch_benchmark (added in b98661d, issue #1) was never being run in\nCI, so its numbers weren't landing on the tracked benchmark history.\ncargo bench without an explicit --bench filter also tries to run the\nlib's own unit-test binary as a benchable target, which doesn't\nunderstand criterion's --output-format flag and errors — needed to\nname both bench targets explicitly instead.",
          "timestamp": "2026-08-11T07:35:09+12:00",
          "tree_id": "bf50cd53dc6e3d00079482cc764fe424042df6bd",
          "url": "https://github.com/Jonesie/costree/commit/df72f9f10109b4acbc0795e50b4ff3a899d5274e"
        },
        "date": 1786390701563,
        "tool": "cargo",
        "benches": [
          {
            "name": "scan_synthetic_tree",
            "value": 1428924,
            "range": "± 82352",
            "unit": "ns/iter"
          },
          {
            "name": "search_narrow_500k",
            "value": 19407440,
            "range": "± 213448",
            "unit": "ns/iter"
          },
          {
            "name": "search_broad_500k_uncapped",
            "value": 341424355,
            "range": "± 11198492",
            "unit": "ns/iter"
          },
          {
            "name": "search_broad_500k_capped",
            "value": 16373184,
            "range": "± 358463",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "peter@jonesie.net.nz",
            "name": "Jonesie",
            "username": "Jonesie"
          },
          "committer": {
            "email": "peter@jonesie.net.nz",
            "name": "Jonesie",
            "username": "Jonesie"
          },
          "distinct": true,
          "id": "646f4202916eb8eaadcf0eb8d32fd91de333b4ef",
          "message": "Fix search matches inside dotfiles being silently hidden\n\nBug report: searching \"aws\" found 1 match but showed nothing in the\nlist. Root cause: with \"Hide dotfiles\" on (persisted in the reporter's\nconfig), render_entry() dropped any entry starting with '.' before\never checking whether it was a search match — so a match living under\na dotfile (here, something inside ~/.aws) got counted by\nsearch_index() but never rendered. The only thing that showed was its\nnon-dotfile ancestor, which is what \"found 1 item, nothing in the\nlist\" actually was: 1 ancestor row, 0 match rows.\n\nHide dotfiles now only applies when there's no active search — an\nexplicit search match should never be silently swallowed by an\nunrelated filter. Added a regression test constructing exactly this\nshape (a match inside a dotfile dir) and asserting it renders.",
          "timestamp": "2026-08-11T08:03:17+12:00",
          "tree_id": "d9458e41d849e77211e720e385b813ee8e0b4416",
          "url": "https://github.com/Jonesie/costree/commit/646f4202916eb8eaadcf0eb8d32fd91de333b4ef"
        },
        "date": 1786392383714,
        "tool": "cargo",
        "benches": [
          {
            "name": "scan_synthetic_tree",
            "value": 1535958,
            "range": "± 69216",
            "unit": "ns/iter"
          },
          {
            "name": "search_narrow_500k",
            "value": 19349904,
            "range": "± 1185821",
            "unit": "ns/iter"
          },
          {
            "name": "search_broad_500k_uncapped",
            "value": 462134252,
            "range": "± 21021151",
            "unit": "ns/iter"
          },
          {
            "name": "search_broad_500k_capped",
            "value": 18275645,
            "range": "± 445051",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}