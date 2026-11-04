pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import NashiApp

// Web download dialog (`nashi-cli download`).
ApplicationWindow {
    id: root

    property string selectedKind: "auto"
    property bool readClipboardOnFocus: false
    property bool ytdlpMissing: App.isToolMissing("yt-dlp")
    property bool commandCopied: false

    readonly property string trimmedUrl: urlField.text.trim()
    readonly property bool urlValid: App.isValidWebUrl(trimmedUrl)
    readonly property bool canSubmit: urlValid && dirField.text.trim().length > 0

    readonly property var kinds: [
        { id: "auto", label: qsTr("Auto"), icon: "starred" },
        { id: "video", label: qsTr("Video"), icon: "video-x-generic" },
        { id: "audio", label: qsTr("Audio (MP3)"), icon: "audio-x-generic" },
        { id: "thumbnail", label: qsTr("Thumbnail"), icon: "image-x-generic" },
        { id: "image", label: qsTr("Direct file"), icon: "folder-download" }
    ]

    function openFor(initialUrl, fromClipboard) {
        commandCopied = false
        ytdlpMissing = App.isToolMissing("yt-dlp")
        if (dirField.text.trim().length === 0)
            dirField.text = App.defaultDownloadDir()

        const given = initialUrl ? String(initialUrl).trim() : ""
        if (given.length > 0) {
            urlField.text = given
            readClipboardOnFocus = false
        } else {
            readClipboardOnFocus = Boolean(fromClipboard)
            const clip = App.clipboardWebUrl()
            if (clip.length > 0) {
                urlField.text = clip
                readClipboardOnFocus = false
            } else if (fromClipboard) {
                urlField.text = ""
            }
        }
        urlField.forceActiveFocus()
        urlField.selectAll()
    }

    function pasteLink() {
        const text = App.clipboardText()
        if (text.length > 0) {
            urlField.text = text
            readClipboardOnFocus = false
        }
        urlField.forceActiveFocus()
    }

    function submit() {
        if (!canSubmit)
            return false
        const ok = App.startDownload(trimmedUrl, selectedKind, dirField.text.trim())
        if (ok)
            root.close()
        return ok
    }

    visible: false
    width: 560
    height: Math.max(460, Math.min(contentCol.implicitHeight + 56, 620))
    minimumWidth: 480
    minimumHeight: 430
    title: qsTr("Download from Link")
    color: Theme.canvas

    palette.window: Theme.canvas
    palette.windowText: Theme.text
    palette.base: Theme.surface
    palette.alternateBase: Theme.inset
    palette.text: Theme.text
    palette.button: Theme.surface
    palette.buttonText: Theme.text
    palette.highlight: Theme.gold
    palette.highlightedText: Theme.buttonInk
    palette.placeholderText: Theme.tertiaryText
    palette.mid: Theme.border

    Component.onCompleted: {
        dirField.text = App.defaultDownloadDir()
        ytdlpMissing = App.isToolMissing("yt-dlp")
    }

    onActiveChanged: {
        if (active && (readClipboardOnFocus || urlField.text.trim().length === 0)) {
            const clip = App.clipboardWebUrl()
            if (clip.length > 0) {
                urlField.text = clip
                urlField.selectAll()
                readClipboardOnFocus = false
            }
        }
    }

    Connections {
        target: App.deps
        function onChanged() {
            root.ytdlpMissing = App.isToolMissing("yt-dlp")
        }
    }

    FolderDialog {
        id: folderDialog
        title: qsTr("Download destination")
        onAccepted: dirField.text = App.settings.urlToLocalPath(selectedFolder)
    }

    Shortcut { sequence: "Escape"; onActivated: root.close() }
    Shortcut {
        sequences: ["Return", "Enter"]
        enabled: root.canSubmit
        onActivated: root.submit()
    }

    ScrollView {
        id: scroll
        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            id: contentCol
            x: 24
            width: Math.max(200, scroll.availableWidth - 48)
            spacing: 14

            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: 20
                spacing: 12

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4

                    Pill {
                        text: qsTr("WEB DOWNLOAD")
                        iconName: "folder-download"
                        tint: Theme.gold
                        textColor: Theme.goldInk
                    }
                    Text {
                        Layout.fillWidth: true
                        text: qsTr("Save from a link.")
                        font.pixelSize: Theme.sizeTitle
                        font.weight: Font.Bold
                        font.letterSpacing: -0.5
                        color: Theme.text
                    }
                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.Wrap
                        text: qsTr("Paste a video, audio or direct file link — saved straight to your folder and added to your Library.")
                        font.pixelSize: Theme.sizeSmall + 1
                        color: Theme.secondaryText
                    }
                }
                NashiMark {
                    size: 56
                    happy: root.urlValid
                }
            }

            // Friendly missing yt-dlp banner
            Rectangle {
                id: ytdlpBanner
                objectName: "ytdlpBanner"
                Layout.fillWidth: true
                visible: root.ytdlpMissing
                implicitHeight: bannerCol.implicitHeight + 28
                radius: Theme.radiusRow
                color: Theme.sand
                border.width: 1
                border.color: Qt.rgba(Theme.goldInk.r, Theme.goldInk.g, Theme.goldInk.b, 0.25)

                ColumnLayout {
                    id: bannerCol
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 14
                    spacing: 8

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8
                        Pill {
                            text: qsTr("MISSING TOOL")
                            tint: Theme.goldInk
                        }
                        Text {
                            Layout.fillWidth: true
                            wrapMode: Text.Wrap
                            text: qsTr("yt-dlp is not installed — needed for video, audio and thumbnail sites.")
                            font.pixelSize: Theme.sizeSmall + 1
                            font.weight: Font.DemiBold
                            color: Theme.text
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8
                        Rectangle {
                            Layout.fillWidth: true
                            implicitHeight: 32
                            radius: Theme.radiusCompact
                            color: Theme.surface
                            border.width: 1
                            border.color: Theme.border
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.margins: 10
                                text: "sudo pacman -S yt-dlp"
                                font.family: "monospace"
                                font.pixelSize: Theme.sizeSmall + 1
                                color: Theme.text
                                elide: Text.ElideRight
                            }
                        }
                        NButton {
                            objectName: "copyYtdlpInstallButton"
                            compact: true
                            kind: "secondary"
                            iconName: "edit-copy"
                            text: root.commandCopied ? qsTr("Copied") : qsTr("Copy install command")
                            onClicked: {
                                App.actions.copyText("sudo pacman -S yt-dlp")
                                root.commandCopied = true
                            }
                        }
                    }
                }
            }

            // Link input
            SectionBox {
                title: qsTr("Link")
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    TextField {
                        id: urlField
                        objectName: "downloadUrlField"
                        Layout.fillWidth: true
                        placeholderText: qsTr("https://example.com/watch?v=…")
                        selectByMouse: true
                        onActiveFocusChanged: {
                            if (activeFocus && text.trim().length === 0) {
                                const clip = App.clipboardWebUrl()
                                if (clip.length > 0)
                                    text = clip
                            }
                        }
                    }
                    NButton {
                        objectName: "downloadPasteButton"
                        compact: true
                        kind: "secondary"
                        iconName: "edit-paste"
                        text: qsTr("Paste")
                        onClicked: root.pasteLink()
                    }
                }
                Text {
                    Layout.fillWidth: true
                    visible: root.trimmedUrl.length > 0 && !root.urlValid
                    wrapMode: Text.Wrap
                    font.pixelSize: Theme.sizeSmall
                    color: Theme.danger
                    text: qsTr("Only http:// and https:// links are supported.")
                }
            }

            // Kind selector
            SectionBox {
                title: qsTr("Format")
                Flow {
                    Layout.fillWidth: true
                    spacing: 8
                    Repeater {
                        model: root.kinds
                        delegate: NButton {
                            id: kindBtn
                            required property var modelData
                            objectName: "downloadKind_" + modelData.id
                            compact: true
                            kind: root.selectedKind === modelData.id ? "primary" : "secondary"
                            iconName: modelData.icon
                            text: modelData.label
                            onClicked: root.selectedKind = kindBtn.modelData.id
                        }
                    }
                }
            }

            // Destination folder
            SectionBox {
                title: qsTr("Save to")
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    TextField {
                        id: dirField
                        objectName: "downloadDirField"
                        Layout.fillWidth: true
                        placeholderText: qsTr("Choose destination folder")
                        selectByMouse: true
                    }
                    NButton {
                        compact: true
                        kind: "secondary"
                        iconName: "folder-open"
                        text: qsTr("Choose…")
                        onClicked: folderDialog.open()
                    }
                }
            }

            // Footer buttons
            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: 2
                Layout.bottomMargin: 20
                spacing: 10
                Text {
                    Layout.fillWidth: true
                    font.pixelSize: Theme.sizeTiny
                    color: Theme.tertiaryText
                    text: qsTr("Existing files are never overwritten.")
                    elide: Text.ElideRight
                }
                NButton {
                    kind: "quiet"
                    text: qsTr("Cancel")
                    onClicked: root.close()
                }
                NButton {
                    objectName: "downloadSubmitButton"
                    kind: "primary"
                    iconName: "folder-download"
                    text: qsTr("Download")
                    enabled: root.canSubmit
                    onClicked: root.submit()
                }
            }
        }
    }
}
