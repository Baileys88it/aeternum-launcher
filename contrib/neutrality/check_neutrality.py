#!/usr/bin/env python3
"""Neutrality guard: fails when a file contains server-specific references.

Checks
  * forbidden words, stored only as SHA-256 hashes of lower-case tokens
    (so this file never spells them out);
  * Windows drive paths outside C:\\Program Files style examples and user profile paths;
  * IPv4 addresses other than loopback, unspecified and documentation ranges;
  * battle.net login hostnames;
  * e-mail addresses other than noreply / example addresses.

Usage
  check_neutrality.py --all              scan every tracked file
  check_neutrality.py --staged           scan staged files (pre-commit)
  check_neutrality.py --range A..B       scan files changed between two commits
Paths listed in .github/neutrality-allow.txt (glob, one per line) are skipped,
for example third-party code under dep/ that is identical to upstream.
"""
import fnmatch, hashlib, ipaddress, os, re, subprocess, sys

FORBIDDEN_TOKEN_SHA256 = {
    "0ac3f61a9e80550400b84bd3af3be949fe0c2c86ffe7a27689d7c52676d1ca0b",
    "2159433528245352a3663c5405452df39cace0656e871a5c76d48e1261373c09",
    "3a7472e71613c0862890009f7cd52fdbf8466dc783148bfd4179fb2b05968b03",
    "3f4b3bd4face125cb417d5fb7f6e2c1717e1d1a0f2372a5633cc3269a0eea54f",
    "43ccb4cad6f08901c3d4dd9560800f9de05b3ed5a87ff820159c14223219aa96",
    "5d5e39a4f3a616605b79647e7aac45d19843a5e648a21218ffe170c5bc611187",
    "63ac23ead531b2d21abb9bab531b3255c6898d818714243a9d70f2c57e8e841a",
    "94aeffe488eeac1a7b31a09ec0bcf7bfadd0cfa508142272d3c45fc0d4b96579",
    "99392b78861b7554e1a0d9e9c19bd06c8828dbbebe517d449ce282612c43c26e",
    "b578682e1aa022c556e53af25721c221024a95a911eddfc3b9d26bc4ac955334",
    "c394a79efe6caa917e12b53932e4d6ac688d7ccc31167e85c5234478befd10df",
    "d6e075ad15048c12f2974d121a25c3d9fc5898072c7a4236fe5245d342a41c7d",
    "fdc104387d48faa98922b462c968f4feb1ccd8498c42356a95c87aefc7ab600c",
}

TOKEN_RE = re.compile(r"[a-z0-9_]+")
DRIVE_RE = re.compile(r"(?<![A-Za-z0-9])(?:[D-Zd-z]:[\\/]+[A-Za-z_]|[Cc]:[\\/]+Users[\\/])")
IPV4_RE = re.compile(r"(?<![\d.])(?:\d{1,3}\.){3}\d{1,3}(?![\d.])")
HOST_RE = re.compile(r"[a-z0-9-]+\.actual\.battle\.net", re.I)
MAIL_RE = re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}")
MAIL_OK = re.compile(r"(noreply|no-reply)@|@(example\.(com|org|net|invalid)|users\.noreply\.github\.com)$|\.(png|jpe?g|gif|svg|webp|ico)$", re.I)
DOC_NETS = [ipaddress.ip_network(n) for n in ("192.0.2.0/24", "198.51.100.0/24", "203.0.113.0/24")]


def git(*args):
    return subprocess.run(["git", *args], capture_output=True, text=True, check=True).stdout


def load_allow(root):
    path = os.path.join(root, ".github", "neutrality-allow.txt")
    if not os.path.exists(path):
        return []
    with open(path, encoding="utf-8") as fh:
        return [l.strip() for l in fh if l.strip() and not l.startswith("#")]


def ip_is_problem(text):
    try:
        ip = ipaddress.ip_address(text)
    except ValueError:
        return False
    if ip.is_loopback or ip.is_unspecified or str(ip).startswith("0.") or str(ip).startswith("255."):
        return False
    return not any(ip in n for n in DOC_NETS)


def check_text(text):
    problems = []
    low = text.lower()
    for tok in set(TOKEN_RE.findall(low)):
        parts = {tok, *tok.split("_")}
        for p in parts:
            if p and hashlib.sha256(p.encode()).hexdigest() in FORBIDDEN_TOKEN_SHA256:
                problems.append("forbidden word (hash match)")
                break
    for rx, label in ((DRIVE_RE, "drive or user-profile path"), (HOST_RE, "battle.net login hostname")):
        if rx.search(text):
            problems.append(label)
    if any(ip_is_problem(m.group(0)) for m in IPV4_RE.finditer(text)):
        problems.append("IPv4 address")
    if any(not MAIL_OK.search(m.group(0)) for m in MAIL_RE.finditer(text)):
        problems.append("e-mail address")
    return sorted(set(problems))


def main(argv):
    root = git("rev-parse", "--show-toplevel").strip()
    os.chdir(root)
    if "--staged" in argv:
        files = git("diff", "--cached", "--name-only", "--diff-filter=ACMR").split("\n")
    elif "--range" in argv:
        rng = argv[argv.index("--range") + 1]
        files = git("diff", "--name-only", "--diff-filter=ACMR", rng).split("\n")
    else:
        files = git("ls-files").split("\n")
    allow = load_allow(root)
    bad = 0
    for f in filter(None, files):
        if any(fnmatch.fnmatch(f, g) for g in allow) or not os.path.isfile(f):
            continue
        with open(f, "rb") as fh:
            data = fh.read()
        if b"\0" in data[:8192]:
            continue  # binary
        text = data.decode("utf-8", errors="replace")
        # the file name itself counts too
        probs = check_text(f + "\n" + text)
        if probs:
            bad += 1
            print(f"NEUTRALITY: {f}: {', '.join(probs)}")
    if bad:
        print(f"\n{bad} file(s) contain server-specific references. Remove them (see CONTRIBUTING.md).")
        return 1
    print("neutrality check passed")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
