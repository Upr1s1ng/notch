# Windows Deployment & Trust Guide (Nusa-Style MSIX + Signing)

## Why Windows Blocks Unsigned GitHub `.exe` Downloads
Windows 11 enforces two layers of execution trust:
1. **Defender SmartScreen:** Warns on downloaded executables that lack a Microsoft-trusted Authenticode signature or established SmartScreen reputation.
2. **Smart App Control (SAC):** When set to **On**, SAC blocks *all* unsigned executables (including `.exe` files built locally or downloaded from GitHub) at the kernel level.

Apps like **Nusa** (`citoxstd.Nusa`) avoid both blocks by packaging as a `Windows.FullTrustApplication` **MSIX** (`runFullTrust`) and distributing through the **Microsoft Store**, where Microsoft re-signs the MSIX package with a Microsoft Trusted Root CA.

---

## Option 1: Microsoft Store Distribution (Zero Cost per Binary, Works with SAC = On)
Every CI run in `.github/workflows/release.yml` builds `dist/Notch_x64.msix` using `msix/AppxManifest.xml` and `msix/pack-msix.ps1`:
1. Register a Microsoft Partner Center developer account.
2. Reserve the app name and copy your assigned **Package Identity `Name`** and **`Publisher`** (`CN=...`) into [msix/AppxManifest.xml](../msix/AppxManifest.xml).
3. Upload `dist/Notch_x64.msix` from the latest GitHub Release to Partner Center.
4. Once certified, Microsoft signs the Store MSIX package so every Windows 10/11 user can install and run it with **Smart App Control = On** and zero SmartScreen warnings.

---

## Option 2: Direct GitHub Release Signing (`WINDOWS_CERT_PFX`)
To make `notch.exe` and `Notch_x64.msix` downloadable directly from GitHub Releases without SmartScreen/SAC prompts:
1. Obtain a Windows Code Signing certificate (EV/OV `.pfx`, Azure Trusted Signing, or [SignPath.io](https://signpath.io/) for open-source projects).
2. Encode the `.pfx` as base64 and add two repository secrets in **Settings → Secrets and variables → Actions**:
   - `WINDOWS_CERT_PFX` — base64-encoded `.pfx` file
   - `WINDOWS_CERT_PASSWORD` — certificate password
3. Push a version tag (`git tag v0.1.0 && git push origin v0.1.0`). CI will automatically sign both `notch.exe` and `Notch_x64.msix`, generate `SHA256SUMS.txt` + GitHub Build Provenance Attestations, and publish a GitHub Release.
