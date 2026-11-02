# Distribution, signing and antivirus false positives

Why new Windows apps get flagged, and the plan to make Wallgates download and install without warnings.

## Why it happens

| Warning | Cause | Fixed by |
| --- | --- | --- |
| Chrome "uncommon / dangerous download" | Reputation: few people have downloaded this exact file yet. Zip files of executables (CI artifacts) score worse | Plain release assets (done), more downloads, code signing |
| SmartScreen "Windows protected your PC" | Unsigned, or signed with a certificate without reputation yet | Code signing + time/downloads |
| Antivirus (ESET, Kaspersky, …) heuristics | Unsigned NSIS installers that extract helper DLLs to `%TEMP%` look like droppers | Signing, false-positive reports, MSI/Store distribution |

Nothing in Wallgates behaves like malware: no hooks, no services, no code injection, no admin rights,
no executables in wallpapers. The warnings are about **reputation**, not behaviour.

## Done in v0.2

- Releases are published as **plain GitHub Release assets** with SHA-256 checksums (no zipped artifacts).
- Full **publisher, product, copyright and version metadata** in the executables and installers.
- **Per-user install** (no UAC prompt), no Windows service, autostart via the standard Run key.
- No UPX or other packers.
- Branded NSIS and MSI installers.

## To do (needs the project owner)

These steps need the owner's identity, so they can't be automated.

### 1. Free code signing: SignPath Foundation (recommended first)
- Free for OSI-licensed open-source projects (Wallgates is GPL-3.0).
- Apply at <https://signpath.org/apply>. Requirements: public repo, builds on GitHub Actions (done),
  MFA on the GitHub account, a short "code signing policy" section in the README.
- The certificate is issued to "SignPath Foundation", and the app is still shown as Wallgates.
- Once accepted, add a signing step to `.github/workflows/build.yml` with
  `signpath/github-action-submit-signing-request` (SignPath provides the exact config).

### 2. Alternative: Certum Open Source certificate (~€49–69/year)
- <https://shop.certum.eu> → "Open Source Code Signing" (cloud key, SimplySign).
- Shows *your* name as publisher. Note the 460-day maximum validity from March 2026: reputation restarts with each new certificate.

### 3. Microsoft Store (free for individuals)
- Register at <https://storedeveloper.microsoft.com> (free for individual developers).
- Store-delivered apps never trigger SmartScreen, and the Store signs MSIX packages for you.
- This is also the natural place for a paid "supporter" edition while the GitHub build stays free.

### 4. winget
- After a signed release: submit a manifest to <https://github.com/microsoft/winget-pkgs>
  so users can run `winget install Wallgates`.

### 5. Report false positives (free, do it for every release)
- **Microsoft Defender / SmartScreen**: <https://www.microsoft.com/en-us/wdsi/filesubmission>
  → "Software developer" → upload the `.exe` and `.msi`.
- **ESET**: e-mail `samples@eset.com` with subject "False positive", attach the installer in a
  password-protected zip (password `infected`) and a link to the release (ESET KB141).
- **Others**: check the files on <https://www.virustotal.com> and use each vendor's
  false-positive form for any detection.

### Do not buy
- **EV certificates**: since 2024 they no longer give instant SmartScreen reputation, so the extra cost isn't worth it.
