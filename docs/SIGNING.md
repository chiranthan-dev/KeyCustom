# Code signing

Browsers and SmartScreen warn about unsigned Windows programs, and block new ones outright more often. KeyForge is more prone to this than most apps because it bundles AutoHotkey, a keyboard-automation tool that heuristics like to flag. The way to stop the warnings is to sign `keyforge.exe` and the installer with a code-signing certificate. CI does that automatically as soon as credentials are configured. Without them it builds unsigned, adds a warning to the run, and still writes `SHA256SUMS.txt` next to the installer.

A signed installer is trusted immediately for its publisher name. SmartScreen reputation still builds up over the first downloads, faster for some certificate types than others.

## Choosing a certificate

| Option | Rough cost | Works in CI | Notes |
|---|---|---|---|
| **Azure Trusted Signing** (recommended) | about US$10/month | Yes, natively | Microsoft-managed certificate in a cloud HSM. Requires identity validation; check Microsoft's current eligibility rules for individuals in your country. |
| OV certificate on a cloud HSM (e.g. a CA's cloud-signing service) | US$100–400/year | With the CA's signing tool | Tauri can call any signing tool through `signCommand`. |
| OV certificate on a USB token | US$70–300/year | No | Since 2023 new certificates must stay on a hardware token, so they cannot be exported to a `.pfx` file for CI. Sign locally instead. |
| Existing `.pfx` (issued before the hardware rule, or self-managed) | — | Yes | Supported below. Self-signed certificates do **not** stop the warnings. |
| Free programs for open-source projects (e.g. SignPath Foundation) | free | Through their service | Requires a public open-source repository and their approval. |

## Option A: Azure Trusted Signing

1. In the Azure portal, create a Trusted Signing account, complete identity validation, and create a certificate profile.
2. Create an app registration (service principal) and give it the **Trusted Signing Certificate Profile Signer** role on the account.
3. In the GitHub repository, open **Settings → Secrets and variables → Actions** and add:
   - **Secrets:** `AZURE_TENANT_ID`, `AZURE_CLIENT_ID` and `AZURE_CLIENT_SECRET` (from the app registration).
   - **Variables:**
     - `TRUSTED_SIGNING_ENDPOINT`, the region endpoint, e.g. `https://eus.codesigning.azure.net`;
     - `TRUSTED_SIGNING_ACCOUNT`, the account name;
     - `TRUSTED_SIGNING_PROFILE`, the certificate profile name.

The next CI run installs `trusted-signing-cli`. It then has Tauri sign the app and the installer, and fails if either signature is invalid.

## Option B: a `.pfx` certificate file

Add two repository secrets:

- `WINDOWS_CERTIFICATE`: the `.pfx` file, base64-encoded. In PowerShell:

```powershell
[Convert]::ToBase64String([IO.File]::ReadAllBytes("cert.pfx")) | Set-Clipboard
```

- `WINDOWS_CERTIFICATE_PASSWORD`: its password.

CI imports the certificate, signs with SHA-256 and a DigiCert timestamp, and checks the signatures. If both options are configured, Azure Trusted Signing is used.

## Releases

Pushing a tag such as `v0.1.0` builds, signs (if configured) and publishes a GitHub release with the installer and `SHA256SUMS.txt`:

```bash
git tag v0.1.0
```

```bash
git push origin v0.1.0
```

Alternatively, run the CI workflow manually (**Actions → CI → Run workflow**) with **release_tag** set, e.g. `v0.1.1`. The release and its tag are created on the commit that was built.
