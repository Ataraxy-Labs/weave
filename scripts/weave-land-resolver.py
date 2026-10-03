#!/usr/bin/env python3
"""Example resolver for `weave land --resolver`: asks any CLI model to resolve one file.

    weave land --resolver 'WEAVE_LAND_MODEL="<your model cli>" python3 scripts/weave-land-resolver.py'

WEAVE_LAND_MODEL is a shell command that reads a prompt on stdin and prints the model's reply on
stdout -- whatever CLI you use; nothing here is tied to one vendor. The prompt asks for the file
between two sentinel lines; this script extracts it and prints it, or prints DELETE / KEEP /
CANNOT, which is the protocol `weave land` reads.

stdin (from weave land): {"path", "kind", "base", "ours", "theirs", "conflicted", "attempt",
"previous", "findings"}; absent sides are null. Standard library only.
"""
import json
import os
import subprocess
import sys

BEGIN, END = "@@@RESOLVED-FILE-BEGIN@@@", "@@@RESOLVED-FILE-END@@@"

KIND = {
    "content": "Both sides edited this file and git could not merge some regions. CONFLICTED is "
               "git's merge attempt: the regions it could not merge are between `<<<<<<<` (OURS), "
               "`=======` and `>>>>>>>` (THEIRS) marker lines; everything outside the markers git "
               "merged automatically.",
    "add/add": "Both sides added this file independently; it does not exist in BASE. CONFLICTED is "
               "git's merge attempt, with marker lines around the regions that differ.",
    "modify/delete": "One side deleted this file and the other modified it ({deleter} deleted it). "
                     "Decide whether the merged branch keeps the file (with whatever edits the "
                     "combination needs) or deletes it.",
}


def frame(name, text):
    if text is None:
        return f"##### {name} BEGIN #####\n(this file does not exist in {name})\n##### {name} END #####\n"
    return f"##### {name} BEGIN #####\n{text if text.endswith(chr(10)) else text + chr(10)}##### {name} END #####\n"


def prompt(job):
    kind = job["kind"]
    deleter = "OURS" if job["ours"] is None else "THEIRS"
    moddel = kind == "modify/delete"
    p = [
        "Resolve the git merge conflict in one file.",
        "",
        f"Path: {job['path']}",
        KIND[kind].format(deleter=deleter),
        "",
        "BASE is the common ancestor. OURS is the target branch. THEIRS is the branch being merged.",
        "Write the merged file: keep every change each side made relative to BASE, combined so the "
        "result is coherent and would work. Do not make changes neither side made, except the "
        "minimum needed to make the combination work. Do not leave conflict markers.",
        "",
        "Answer with exactly one of these, and nothing else:",
        f"(a) the complete resolved file, every line, between the lines {BEGIN} and {END};",
    ]
    if moddel:
        p.append("(b) the single line DELETE if the file should be deleted;")
    p += ["(c) CANNOT: <one-line reason> if you cannot produce a correct resolution.",
          "Do not wrap the file in code fences and do not abbreviate anything.", "",
          frame("BASE", job["base"]), frame("OURS", job["ours"]), frame("THEIRS", job["theirs"])]
    if job.get("conflicted") is not None:
        p.append(frame("CONFLICTED", job["conflicted"]))
    if job.get("previous") is not None:
        p += ["A previous answer was rejected by an automatic verifier. Its findings:"]
        p += [f"- {f}" for f in job.get("findings") or []]
        p += ["Answer again in the same format, fixing these problems, or answer CANNOT.",
              frame("PREVIOUS-ANSWER", job["previous"])]
    return "\n".join(p) + "\n"


def main():
    job = json.load(sys.stdin)
    model = os.environ.get("WEAVE_LAND_MODEL")
    if not model:
        print("CANNOT: WEAVE_LAND_MODEL is not set")
        return
    r = subprocess.run(model, shell=True, input=prompt(job), capture_output=True, text=True)
    if r.returncode != 0:
        sys.stderr.write(r.stderr[-500:])
        sys.exit(1)
    text = r.stdout
    i, j = text.find(BEGIN), text.rfind(END)
    if i >= 0 and j > i:
        body = text[i + len(BEGIN):j]
        body = body[2:] if body.startswith("\r\n") else body[1:] if body.startswith("\n") else body
        lines = body.split("\n")
        if lines and lines[0].startswith("```") and not any(
                (s or "").lstrip().startswith("```") for s in (job["ours"], job["theirs"])):
            body = "\n".join(lines[1:])
            if body.rstrip().endswith("```"):
                body = body.rstrip()[:-3]
        sys.stdout.write(body)
        return
    t = text.strip()
    if job["kind"] == "modify/delete" and t.splitlines()[:1] == ["DELETE"]:
        print("DELETE")
    elif t.startswith("CANNOT"):
        print(t.splitlines()[0])
    else:
        print("CANNOT: the model's answer had no sentinel-framed file")


if __name__ == "__main__":
    main()
