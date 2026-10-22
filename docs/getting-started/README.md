# Install OpenFlow

<a id="getting-started"></a>

The macOS release needs no Rust or Node. For a development checkout, see [Build from source](build-from-source.md).

<a id="install-release-build"></a>

## Install the macOS release

1. Download the latest macOS build from [GitHub Releases](https://github.com/philbotar/OpenFlow/releases/latest).
2. Open the `.dmg`, drag **OpenFlow** to **Applications**, and open the app.

If macOS blocks it, right-click **OpenFlow** → **Open**, or run `xattr -cr /path/to/OpenFlow.app` on the `.app` bundle.

<a id="choose-what-to-do-next"></a>

## First launch

An onboarding tour opens. On its last slide, **Set up provider →** opens **Settings → Providers**; then follow [Provider setup](../guides/provider-setup.md). **Help** reopens the tour.

<a id="update-the-packaged-app"></a>

## Update or reinstall

Stop any active workflow or chat run, then select **Update now** in **Settings → About**. If an update is published, OpenFlow downloads it, installs it, and relaunches. Otherwise the button reports that the app is current or that the update failed.

In-app updates work only in the packaged macOS app. For a development build or a failed update, install the latest build from [GitHub Releases](https://github.com/philbotar/OpenFlow/releases/latest) by hand.

<a id="build-from-source"></a>

Development requirements and installer commands: [Build from source](build-from-source.md).
