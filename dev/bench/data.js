window.BENCHMARK_DATA = {
  "lastUpdate": 1786390335608,
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
      }
    ]
  }
}