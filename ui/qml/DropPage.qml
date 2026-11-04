pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import NashiApp

// Home: hero, drop zone, the three action tiles and the selected group's items.
Page {
    id: page

    readonly property Selection sel: App.home
    readonly property bool hasFiles: sel.fileCount > 0
    readonly property string mode: sel.model.mode

    function keyName(key, fallback) {
        const k = key && key.length > 0 ? key : fallback
        return { shift: "⇧", ctrl: "Ctrl", alt: "Alt", meta: "Meta" }[k] || "—"
    }

    background: null
    padding: 0

    // Used by the Home window's Ctrl+O / Ctrl+V shortcuts.
    function chooseFiles() { fileDialog.open() }
    function paste() { App.clipboard.pasteInto(page.sel) }

    FileDialog {
        id: fileDialog
        fileMode: FileDialog.OpenFiles
        title: qsTr("Choose files")
        onAccepted: page.sel.setUrls(selectedFiles)
    }

    ScrollView {
        id: scroll
        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            width: scroll.availableWidth
            spacing: 22

            PageHeading {
                Layout.fillWidth: true
                Layout.leftMargin: 28
                Layout.rightMargin: 28
                Layout.topMargin: 8
                title: page.hasFiles ? qsTr("Let’s make something of it.") : qsTr("Your files. Their next form.")
                subtitle: qsTr("Convert, fine-tune, and send them on their way.")
            }

            // Drop zone
            Rectangle {
                id: zone
                Layout.fillWidth: true
                Layout.leftMargin: 28
                Layout.rightMargin: 28
                implicitHeight: zoneColumn.implicitHeight + 48
                radius: Theme.radiusZone
                color: Theme.surface
                border.width: dropArea.containsDrag ? 2 : 1
                border.color: dropArea.containsDrag ? Theme.gold : Theme.border
                scale: dropArea.containsDrag && Theme.motion ? 1.008 : 1
                Behavior on scale { NumberAnimation { duration: Theme.quick } }

                ColumnLayout {
                    id: zoneColumn
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 24
                    spacing: 18

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 22

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 12

                            Pill {
                                tint: Theme.gold
                                textColor: Theme.goldInk
                                iconName: dropArea.containsDrag ? "go-down" : "starred"
                                text: dropArea.containsDrag ? qsTr("A perfect landing spot")
                                      : page.hasFiles ? qsTr("READY WHEN YOU ARE") : qsTr("START HERE")
                            }
                            Text {
                                Layout.fillWidth: true
                                text: dropArea.containsDrag ? qsTr("Drop it like it’s… a file.")
                                      : !page.hasFiles ? qsTr("Drop something good.")
                                      : page.sel.fileCount === 1 ? App.fileName(page.sel.files[0])
                                      : qsTr("%1 files, ready to go.").arg(page.sel.fileCount)
                                elide: Text.ElideMiddle
                                font.pixelSize: Theme.sizeTitle
                                font.weight: Font.Bold
                                font.letterSpacing: -0.5
                                color: Theme.text
                            }
                            Text {
                                Layout.fillWidth: true
                                text: !page.hasFiles ? qsTr("Images, videos, documents, and all the little things in between.")
                                      : page.sel.fileCount === 1 ? page.sel.files[0].substring(0, page.sel.files[0].lastIndexOf("/"))
                                      : qsTr("Keep them together, or give each one a fresh start.")
                                elide: Text.ElideMiddle
                                font.pixelSize: Theme.sizeSmall + 1
                                color: Theme.secondaryText
                            }
                            RowLayout {
                                Layout.topMargin: 3
                                spacing: 9
                                NButton {
                                    kind: page.hasFiles ? "secondary" : "primary"
                                    iconName: "list-add"
                                    text: page.hasFiles ? qsTr("Add or change files") : qsTr("Choose files")
                                    ToolTip.visible: hovered
                                    ToolTip.text: qsTr("Ctrl+O")
                                    onClicked: page.chooseFiles()
                                }
                                NButton {
                                    objectName: "pasteButton"
                                    kind: "secondary"
                                    iconName: "edit-paste"
                                    text: qsTr("Paste")
                                    ToolTip.visible: hovered
                                    ToolTip.text: qsTr("Ctrl+V · Paste files, text, an image or a link")
                                    onClicked: page.paste()
                                }
                                NButton {
                                    objectName: "downloadLinkButton"
                                    kind: "quiet"
                                    iconName: "folder-download"
                                    text: qsTr("Download link…")
                                    ToolTip.visible: hovered
                                    ToolTip.text: qsTr("Save video, audio or a file from an http(s) link")
                                    onClicked: App.showDownloadFromClipboard()
                                }
                                NButton {
                                    visible: page.hasFiles
                                    kind: "primary"
                                    iconName: "view-refresh"
                                    text: qsTr("Open wheel")
                                    onClicked: App.showWheel(page.sel.files, page.mode)
                                }
                            }
                        }

                        // Pear "garden"
                        Item {
                            Layout.preferredWidth: 150
                            Layout.preferredHeight: 136
                            Rectangle {
                                anchors.centerIn: parent
                                width: 128
                                height: 128
                                radius: 64
                                color: dropArea.containsDrag ? Theme.sand : Theme.sage
                                Behavior on color { ColorAnimation { duration: Theme.normal } }
                            }
                            NashiMark {
                                anchors.centerIn: parent
                                anchors.verticalCenterOffset: dropArea.containsDrag && Theme.motion ? -6 : 0
                                size: 104
                                happy: page.hasFiles || dropArea.containsDrag
                                Behavior on anchors.verticalCenterOffset { NumberAnimation { duration: Theme.normal; easing.type: Easing.OutBack } }
                            }
                        }
                    }

                    // Selected file chips
                    Flow {
                        Layout.fillWidth: true
                        visible: page.hasFiles
                        spacing: 8
                        Repeater {
                            model: page.sel.files.slice(0, 3)
                            delegate: Rectangle {
                                id: chip
                                required property string modelData
                                implicitWidth: chipRow.implicitWidth + 18
                                implicitHeight: chipRow.implicitHeight + 12
                                radius: 9
                                color: Theme.canvas
                                Row {
                                    id: chipRow
                                    anchors.centerIn: parent
                                    spacing: 6
                                    Image {
                                        anchors.verticalCenter: parent.verticalCenter
                                        source: "image://icon/" + App.mimeIcon(chip.modelData)
                                        sourceSize: Qt.size(34, 34)
                                        width: 17
                                        height: 17
                                    }
                                    Text {
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: App.fileName(chip.modelData)
                                        width: Math.min(implicitWidth, 220)
                                        elide: Text.ElideMiddle
                                        font.pixelSize: Theme.sizeTiny
                                        font.weight: Font.Medium
                                        color: Theme.text
                                    }
                                    NButton {
                                        anchors.verticalCenter: parent.verticalCenter
                                        kind: "quiet"
                                        compact: true
                                        leftPadding: 4
                                        rightPadding: 4
                                        topPadding: 2
                                        bottomPadding: 2
                                        iconName: "window-close"
                                        Accessible.name: qsTr("Remove %1").arg(App.fileName(chip.modelData))
                                        onClicked: page.sel.setFiles(page.sel.files.filter(f => f !== chip.modelData))
                                    }
                                }
                            }
                        }
                        Text {
                            visible: page.sel.fileCount > 3
                            height: 29
                            verticalAlignment: Text.AlignVCenter
                            text: "+" + (page.sel.fileCount - 3)
                            font.pixelSize: Theme.sizeSmall
                            color: Theme.secondaryText
                        }
                        NButton {
                            kind: "quiet"
                            compact: true
                            text: qsTr("Clear")
                            onClicked: page.sel.clear()
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.topMargin: 2
                        spacing: 8
                        // Live chord keys from nashi-inputd's config (defaults until known).
                        KeyChip { keyText: page.keyName(App.inputd.convertKey, "shift"); label: qsTr("Convert") }
                        KeyChip { keyText: page.keyName(App.inputd.toolsKey, "ctrl"); label: qsTr("Tools") }
                        KeyChip { keyText: page.keyName(App.inputd.actionsKey, "alt"); label: qsTr("Actions") }
                        Item { Layout.fillWidth: true }
                        Text {
                            text: qsTr("Hold a key while dragging a file")
                            font.pixelSize: Theme.sizeTiny
                            color: Theme.secondaryText
                        }
                    }
                }

                DropArea {
                    id: dropArea
                    anchors.fill: parent
                    onEntered: (drag) => {
                        drag.action = Qt.CopyAction
                        drag.accept(Qt.CopyAction)
                    }
                    onPositionChanged: (drag) => {
                        drag.action = Qt.CopyAction
                        drag.accept(Qt.CopyAction)
                    }
                    onDropped: (drop) => {
                        drop.action = Qt.CopyAction
                        drop.accept(Qt.CopyAction)
                        if (drop.hasUrls)
                            page.sel.setUrls(drop.urls)
                    }
                }
            }

            // Action tiles
            RowLayout {
                Layout.fillWidth: true
                Layout.leftMargin: 28
                Layout.rightMargin: 28
                visible: page.hasFiles
                spacing: 12

                ActionTile {
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    readonly property int n: page.sel.ready ? page.sel.model.countFor("convert") : 0
                    text: qsTr("Convert")
                    subtitle: !page.sel.ready ? qsTr("Reading files…")
                              : n === 0 ? qsTr("No output formats")
                              : n === 1 ? qsTr("1 way to change it") : qsTr("%1 ways to change it").arg(n)
                    iconName: "view-refresh"
                    tint: Theme.gold
                    selected: page.mode === "convert"
                    onClicked: page.sel.model.mode = "convert"
                }
                ActionTile {
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    readonly property int n: page.sel.ready ? page.sel.model.countFor("tools") : 0
                    text: qsTr("Fine-tune")
                    subtitle: page.sel.ready && n === 0 ? qsTr("No tools for this selection") : qsTr("Edit, compress, and more")
                    iconName: "configure"
                    tint: Theme.forest
                    selected: page.mode === "tools"
                    onClicked: page.sel.model.mode = "tools"
                }
                ActionTile {
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    text: qsTr("Move & share")
                    subtitle: qsTr("Send it on its way")
                    iconName: "document-send"
                    tint: Theme.sky
                    selected: page.mode === "actions"
                    onClicked: page.sel.model.mode = "actions"
                }
            }

            BusyIndicator {
                Layout.alignment: Qt.AlignHCenter
                visible: running
                running: page.sel.loading
            }

            Text {
                Layout.fillWidth: true
                Layout.leftMargin: 28
                Layout.rightMargin: 28
                visible: page.sel.error.length > 0
                wrapMode: Text.Wrap
                color: Theme.danger
                text: qsTr("Could not read the file types: %1").arg(page.sel.error)
            }

            Text {
                Layout.fillWidth: true
                visible: page.hasFiles && page.sel.ready && page.sel.model.count === 0
                horizontalAlignment: Text.AlignHCenter
                color: Theme.secondaryText
                text: qsTr("Nothing in this group for these files.")
            }

            ItemGrid {
                Layout.fillWidth: true
                Layout.leftMargin: 22
                Layout.rightMargin: 22
                Layout.preferredHeight: contentHeight
                visible: page.hasFiles && page.sel.ready
                interactive: false
                wheelModel: page.sel.model
                onPicked: (kind, itemId, title) => App.runItem(kind, itemId, title, page.sel.files)
            }

            Item { Layout.preferredHeight: 8 }
        }
    }
}
