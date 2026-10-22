# Install OpenFlow

<a id="getting-started"></a>

Download the macOS release to use OpenFlow without Rust or Node. For a development checkout, use [Build from source](build-from-source.md).

<a id="install-release-build"></a>

## Install the macOS release

Download the latest macOS build from [GitHub Releases](https://github.com/philbotar/OpenFlow/releases/latest). Open the `.dmg`, drag **OpenFlow** to **Applications**, then open the app.

If macOS blocks the app, right-click **OpenFlow** → **Open**, or run `xattr -cr /path/to/OpenFlow.app` on the `.app` bundle.

<a id="choose-what-to-do-next"></a>

## First launch

The app opens its onboarding tour. On the last slide, **Set up provider →** opens **Settings → Providers**. Follow [Provider setup](../guides/provider-setup.md) to make the first connection. You can reopen the tour from **Help**.

<a id="update-the-packaged-app"></a>

## Update or reinstall

Open **Settings → About**, then select **Update now**. Stop any active workflow or chat run first.

When a published update is available, OpenFlow downloads it, installs it, and relaunches. The button reports when the app is current or when an update fails.

In-app updates are available only in the packaged macOS app. For a development build or an update failure, download the latest build from [GitHub Releases](https://github.com/philbotar/OpenFlow/releases/latest) and install it manually.

<a id="build-from-source"></a>

Development requirements and installer commands are in [Build from source](build-from-source.md).
