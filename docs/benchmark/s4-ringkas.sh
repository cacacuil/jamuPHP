#!/usr/bin/env bash
# Meringkas s4-results.txt: median, terendah, dan tertinggi req/dtk dari tiap beban, serta
# median p50 dan p99. Pemakaian: bash s4-ringkas.sh [s4-results.txt]
f=${1:-$(dirname "$0")/s4-results.txt}
grep '|jalan=' "$f" | awk -F'|' '
  function ms(s) { # "305.85ms" -> milidetik
    if (s ~ /us$/) return s / 1000; if (s ~ /ms$/) return s + 0; if (s ~ /m$/) return s * 60000; return s * 1000
  }
  function med(a, n,   i, j, t) { # n kecil: urutkan dengan sisipan
    for (i = 2; i <= n; i++) { t = a[i]; for (j = i - 1; j >= 1 && a[j] > t; j--) a[j + 1] = a[j]; a[j + 1] = t }
    return n % 2 ? a[(n + 1) / 2] : (a[n / 2] + a[n / 2 + 1]) / 2
  }
  {
    k = $2 "|" $3 "|" $4
    if (!(k in n)) urut[++m] = k
    i = ++n[k]
    for (c = 6; c <= NF; c++) { split($c, kv, "="); v[kv[1]] = kv[2] }
    rps[k, i] = v["rps"] + 0; p50[k, i] = ms(v["p50"]); p99[k, i] = ms(v["p99"])
    if (v["non2xx"] + 0 > 0 || v["galat"] != "0") galat[k] = galat[k] " " v["non2xx"] "/" v["galat"]
    delete v
  }
  END {
    printf "%-34s %-44s %-42s %10s %10s %10s %9s %9s  %s\n", "pintu", "beban", "koneksi", "median", "terendah", "tertinggi", "p50 ms", "p99 ms", "non2xx/galat"
    for (x = 1; x <= m; x++) {
      k = urut[x]; split(k, bag, "|"); lo = 1e18; hi = 0
      for (i = 1; i <= n[k]; i++) { a[i] = rps[k, i]; b[i] = p50[k, i]; c2[i] = p99[k, i]; if (a[i] < lo) lo = a[i]; if (a[i] > hi) hi = a[i] }
      printf "%-34s %-44s %-42s %10.1f %10.1f %10.1f %9.2f %9.2f  %s\n", bag[1], bag[2], bag[3], med(a, n[k]), lo, hi, med(b, n[k]), med(c2, n[k]), (k in galat) ? galat[k] : "-"
    }
  }'
grep '|mem|' "$f" | cut -d'|' -f3-
