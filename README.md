# portwatch

A macOS menu bar app that shows your local dev servers in one place and stops them the right way.

## What it does

- Groups many localhost servers that differ only by port into their projects.
- Finds services started with `brew services`, including root services started with `sudo`, without using sudo itself.
- Shows what manages each server (a terminal, a brew service, Docker, pm2, and so on) and stops it through that manager.
- Reads the hosts file and the nginx config to show "domain → nginx → dev server" routes and certificate expiry dates.
- Folds away noise such as security agents and system services by default.

The app UI is in Korean.

## Install

Requires an Apple Silicon Mac with macOS 15 or later. Install with Homebrew:

```bash
brew install --cask superlucky84/tap/portwatch
```

The app is ad-hoc signed but not notarized by Apple, so macOS blocks the first launch. You only need to allow it once:

1. Launch portwatch from Spotlight or `/Applications`. When macOS says it cannot verify the developer, click **Done**.
2. Open **System Settings → Privacy & Security** and click **Open Anyway** near the bottom.
3. Click **Open** in the next prompt and confirm with your Mac password or Touch ID.

You may need to do this again after an upgrade on some macOS versions. If your company's security software blocks unsigned apps,
follow its policy.

Update and uninstall:

```bash
brew upgrade --cask portwatch
```

```bash
brew uninstall --cask portwatch
```

Use `brew uninstall --zap --cask portwatch` to remove the settings folder too.

## Using it

portwatch has no Dock icon. A ring icon appears in the menu bar with the number of dev servers and services next to it.
Left-click opens the list, right-click opens the Quit menu.

Click a row to see details and the Open, Copy URL, Hide, and Pin buttons. Stop is offered only for entries portwatch can stop
safely through their manager. Root services ask for your password in the standard macOS dialog. Settings cover the refresh
interval, launch at login, and the list of security programs. They are stored in
`~/Library/Application Support/io.github.superlucky84.portwatch/settings.json`.

## Documents

This project follows Doc-Driven Development. The planning documents are in Korean.

- [Requirements](./docs/REQUIREMENTS.md)
- [Design](./docs/DESIGN.md)
- [Implementation plan](./docs/IMPLEMENT.md)
- [Manual test checklist](./docs/MANUAL_TEST_CHECKLIST.md)

## Development

Built with Tauri 2. The detection and classification logic (`crates/portwatch-core`) can be tested without macOS.

| Path | Contents |
| --- | --- |
| `crates/portwatch-core/` | Pure Rust code that parses command output and classifies entries |
| `crates/portwatch-collect/` | Runs the macOS commands, plus the `portwatch-probe` diagnostic tool |
| `src-tauri/` | Menu bar icon and popover window |
| `ui/` | Popover UI (HTML/CSS/JS) |

Requirements: Rust (stable) and Node 22 or later. Building the macOS app also needs the Xcode Command Line Tools.

```bash
npm ci
```

Formatting, clippy, and document link checks:

```bash
npm run check
```

Rust tests and script tests:

```bash
npm test
```

UI tests run in Playwright with a fake Tauri object (run `npx playwright install chromium` once first):

```bash
npm run test:ui
```

Build the app on macOS. The result is `target/release/bundle/macos/portwatch.app`:

```bash
npm run build:mac
```

### Running an app built by CI

CI builds the macOS app and runs a real-service detection scenario on every push to `main` and every pull request.
Each run uploads a `portwatch-macos-arm64` artifact (zip) that contains `portwatch.app` and `portwatch-probe`.
You can also start a run from the Actions tab with **CI → Run workflow**.

Downloaded apps are blocked on first launch; allow them as described in [Install](#install). For an app you built yourself,
you can remove the quarantine flag instead:

```bash
xattr -dr com.apple.quarantine /Applications/portwatch.app
```

### Inspecting detection results (`portwatch-probe`)

The artifact also includes `portwatch-probe`, which prints detection results without the UI. Remove the quarantine flag first:

```bash
xattr -d com.apple.quarantine ./portwatch-probe
```

Show a table grouped by category. Home paths are shown as `~`:

```bash
./portwatch-probe
```

Print raw command output for checking formats. User names become `<user>`, home paths become `~`, and only lines about
listening processes and their parents are kept:

```bash
./portwatch-probe --raw
```

`--json` prints the snapshot JSON that the app receives.

## Releasing

The built app zip is published to the Releases of the public tap repository `superlucky84/homebrew-tap`, and
`Casks/portwatch.rb` in that repository points to it.

### One-time setup

- The tap repository `superlucky84/homebrew-tap` exists.
- Create a fine-grained token with **Contents: Read and write** on that repository only, and save it as the Actions secret
  `TAP_TOKEN` in this repository. When the token expires, create a new one and save it under the same name.

### Releasing a new version

1. Make sure `main` passes the checks. Also run `npm run test:ui` if `ui/` changed and `npm run check:app` if `src-tauri/` changed.

   ```bash
   npm run check && npm test
   ```

2. Set the same version in three files: `package.json`, `Cargo.toml` (`[workspace.package]`), and `src-tauri/tauri.conf.json`.
   Replace `0.1.2` with the new version. This updates `package.json` and `package-lock.json`:

   ```bash
   npm version 0.1.2 --no-git-tag-version
   ```

   Edit `version` in `Cargo.toml` and `src-tauri/tauri.conf.json` by hand, refresh `Cargo.lock`, and confirm that
   the versions match the tag:

   ```bash
   cargo metadata --format-version 1 > /dev/null
   ```

   ```bash
   node scripts/release.mjs check v0.1.2
   ```

3. Commit the version change, push `main`, then create and push the tag.

   ```bash
   git tag -a v0.1.2 -m "portwatch 0.1.2"
   ```

   ```bash
   git push origin v0.1.2
   ```

4. The **Release** workflow builds the app on macOS, signs it ad hoc, zips it, creates the `portwatch-v<version>` release
   in the tap repository, and commits the updated cask. It takes about three minutes.

   ```bash
   gh run list --workflow release.yml --limit 1
   ```

5. Check that the tap repository has the new release and that `Casks/portwatch.rb` shows the new version. Then update
   an installed copy:

   ```bash
   brew update && brew upgrade --cask portwatch
   ```

To rehearse without publishing, run **Release → Run workflow** in the Actions tab with `publish` off. It builds the app and
uploads the zip as an artifact only.

If the workflow fails after it created the release in the tap repository, rerunning it fails because the release
already exists. Delete that release in the tap repository first, or fix the problem and release a new version.
