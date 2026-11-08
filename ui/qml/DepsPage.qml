pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

// External tools reported by `nashi-cli deps --json`, grouped into cards
// after the upstream Windows tool setup window. Nashi only reports and
// suggests a pacman command; it never downloads or installs anything.
Page {
    id: page
    background: null
    padding: 0

    signal homeRequested()

    readonly property var groups: [
        { id: "media", title: qsTr("Media tools"),
          detail: qsTr("Audio and video conversion, previews, trimming and web downloads.") },
        { id: "documents", title: qsTr("Document tools"),
          detail: qsTr("Office files, PDF pages, text extraction and compression.") },
        { id: "images", title: qsTr("Image tools"),
          detail: qsTr("Image conversion, editing, metadata, HEIC and SVG.") },
        { id: "more", title: qsTr("More tools"),
          detail: qsTr("Archives, QR codes, text recognition and everything else.") }
    ]
    readonly property var groupOf: ({
        "ffmpeg": "media", "ffprobe": "media", "yt-dlp": "media", "curl": "media",
        "soffice": "documents", "pandoc": "documents", "qpdf": "documents", "pdftoppm": "documents",
        "pdftotext": "documents", "gs": "documents",
        "magick": "images", "exiftool": "images", "libheif": "images", "rsvg-convert": "images", "opencv": "images"
    })
    function toolGroup(toolId) {
        return groupOf[toolId] ?? "more"
    }

    ScrollView {
        id: scroll
        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            x: 28
            width: scroll.availableWidth - 56
            spacing: 14

            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: 8
                PageHeading {
                    Layout.fillWidth: true
                    eyebrow: qsTr("Tools")
                    title: qsTr("A few tools. More possibilities.")
                    subtitle: qsTr("Nashi uses the programs already on your system. Nothing is downloaded.")
                }
                NButton {
                    Layout.alignment: Qt.AlignBottom
                    compact: true
                    iconName: "view-refresh"
                    text: App.deps.loading ? qsTr("Checking…") : qsTr("Check again")
                    enabled: !App.deps.loading
                    onClicked: App.deps.refresh()
                }
            }

            Text {
                Layout.fillWidth: true
                visible: App.cliPath.length === 0
                wrapMode: Text.Wrap
                color: Theme.danger
                text: qsTr("nashi-cli was not found. Install Nashi properly or set NASHI_CLI.")
            }
            Text {
                Layout.fillWidth: true
                visible: App.deps.error.length > 0
                wrapMode: Text.Wrap
                color: Theme.danger
                text: App.deps.error
            }

            // Status and the next step, like the setup window's footer.
            Card {
                Layout.fillWidth: true
                implicitHeight: statusColumn.implicitHeight + 40
                color: App.deps.missingCount > 0 ? Theme.sand : Theme.sage
                border.color: Theme.line

                ColumnLayout {
                    id: statusColumn
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 20
                    spacing: 10

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 2
                            Text {
                                Layout.fillWidth: true
                                wrapMode: Text.Wrap
                                font.pixelSize: Theme.sizeHeading
                                font.weight: Font.DemiBold
                                color: Theme.text
                                text: App.deps.missingCount > 0
                                      ? qsTr("%1 tools missing").arg(App.deps.missingCount)
                                      : qsTr("Ready. You can go back to your files.")
                            }
                            Text {
                                Layout.fillWidth: true
                                wrapMode: Text.Wrap
                                font.pixelSize: Theme.sizeSmall + 1
                                color: Theme.secondaryText
                                text: App.deps.missingCount > 0
                                      ? qsTr("Install them to enable every conversion, preview and download. Tools already on your computer are kept.")
                                      : qsTr("Every conversion, preview and download is available.")
                            }
                        }
                        NButton {
                            objectName: "toolsContinueButton"
                            visible: App.deps.missingCount === 0
                            kind: "primary"
                            text: qsTr("Continue to Nashi")
                            onClicked: page.homeRequested()
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        visible: App.deps.missingCount > 0 && App.deps.installCommand.length > 0
                        NTextField {
                            id: cmd
                            Layout.fillWidth: true
                            readOnly: true
                            font.family: "monospace"
                            text: App.deps.installCommand
                            Accessible.name: qsTr("Install command")
                        }
                        NButton {
                            kind: "primary"
                            compact: true
                            iconName: "edit-copy"
                            text: qsTr("Copy")
                            onClicked: App.actions.copyText(cmd.text)
                        }
                    }
                }
            }

            Repeater {
                model: page.groups
                delegate: SectionBox {
                    id: group
                    required property var modelData
                    objectName: "toolGroup_" + modelData.id
                    title: modelData.title
                    Layout.topMargin: 2

                    Text {
                        Layout.fillWidth: true
                        Layout.topMargin: -6
                        wrapMode: Text.Wrap
                        font.pixelSize: Theme.sizeSmall + 1
                        color: Theme.secondaryText
                        text: group.modelData.detail
                    }

                    Repeater {
                        model: App.deps
                        delegate: RowLayout {
                            id: row
                            required property string toolId
                            required property bool available
                            required property string version
                            required property string path
                            required property string packageName
                            required property bool optional
                            required property string usedFor
                            Layout.fillWidth: true
                            visible: page.toolGroup(toolId) === group.modelData.id
                            spacing: 12

                            Rectangle {
                                Layout.preferredWidth: 10
                                Layout.preferredHeight: 10
                                radius: 5
                                color: row.available ? Theme.success : (row.optional ? Theme.gold : Theme.danger)
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 1
                                Text {
                                    Layout.fillWidth: true
                                    text: row.toolId + (row.optional ? qsTr(" (optional)") : "")
                                    font.pixelSize: Theme.sizeBody
                                    font.weight: Font.DemiBold
                                    color: Theme.text
                                }
                                Text {
                                    Layout.fillWidth: true
                                    elide: Text.ElideRight
                                    font.pixelSize: Theme.sizeSmall
                                    color: Theme.secondaryText
                                    text: row.available
                                          ? qsTr("%1 · %2").arg(row.version.length > 0 ? row.version : qsTr("installed")).arg(row.path)
                                          : qsTr("Missing — package: %1").arg(row.packageName)
                                }
                                Text {
                                    Layout.fillWidth: true
                                    visible: row.usedFor.length > 0
                                    elide: Text.ElideRight
                                    font.pixelSize: Theme.sizeSmall
                                    color: Theme.tertiaryText
                                    text: qsTr("Used for: %1").arg(row.usedFor)
                                }
                            }
                            Pill {
                                text: row.available ? qsTr("Ready") : row.optional ? qsTr("Optional") : qsTr("Missing")
                                tint: row.available ? Theme.success : row.optional ? Theme.gold : Theme.danger
                                textColor: row.available ? Theme.forest : row.optional ? Theme.goldInk : Theme.danger
                            }
                        }
                    }
                }
            }

            Item { Layout.preferredHeight: 8 }
        }
    }
}
