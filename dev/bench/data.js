window.BENCHMARK_DATA = {
  "lastUpdate": 1786373351569,
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
      }
    ]
  }
}