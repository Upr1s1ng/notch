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

## Option 2: Azure Artifact Signing (Recommended for Direct GitHub `.exe` / `.msix` Downloads)
`.github/workflows/release.yml` is wired for **Azure Artifact Signing** (`azure/artifact-signing-action@v2` + OIDC):
1. Create an **Artifact Signing** account, complete identity validation, and create a Public Trust certificate profile in the Azure Portal.
2. Create an Entra ID App Registration with a **Federated Credential** for `Upr1s1ng/notch` (`main` branch and/or tags) and assign it the **Artifact Signing Certificate Profile Signer** role.
3. In GitHub **Settings → Secrets and variables → Actions**:
   - **Variables:**
     - `AZURE_SIGNING_ENDPOINT` (e.g. `https://eus.codesigning.azure.net/`)
     - `AZURE_SIGNING_ACCOUNT` (your Artifact Signing account name)
     - `AZURE_SIGNING_PROFILE` (your certificate profile name)
   - **Secrets:**
     - `AZURE_CLIENT_ID`
     - `AZURE_TENANT_ID`
     - `AZURE_SUBSCRIPTION_ID`
4. Push a version tag (`git tag v0.1.1 && git push origin v0.1.1`) — CI will sign both `notch.exe` and `Notch_x64.msix` automatically.

---

## Option 3: Direct PFX Certificate (`WINDOWS_CERT_PFX`)
If you already have an EV/OV `.pfx` code-signing certificate:
1. Encode the `.pfx` as base64 and add two repository **Secrets** in **Settings → Secrets and variables → Actions**:
   - `WINDOWS_CERT_PFX` — base64-encoded `.pfx` file
   - `WINDOWS_CERT_PASSWORD` — certificate password
2. Push a version tag — CI will sign both `notch.exe` and `Notch_x64.msix` with `signtool.exe`.
