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
        return { shift: "Shift", ctrl: "Ctrl", alt: "Alt", meta: "Meta" }[k] || "—"
    }

    background: null
    padding: 0

    // Handled by HomeWindow: switch tab or open the shortcuts sheet.
    signal libraryRequested()
    signal toolsRequested()
    signal shortcutsRequested()

    // Existing recent files matching the Home search, newest first.
    property int recentRevision: 0
    readonly property var recentHits: {
        page.recentRevision
        return App.filteredRecentPaths(recentSearch.text)
    }
    // Library files still on disk; Home only lists those.
    readonly property int recentTotal: {
        page.recentRevision
        return App.filteredRecentPaths("").length
    }
    Connections {
        target: App.recent
        function onRowsInserted() { page.recentRevision++ }
        function onRowsRemoved() { page.recentRevision++ }
        function onModelReset() { page.recentRevision++ }
        function onDataChanged() { page.recentRevision++ }
    }

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
                                    visible: page.hasFiles
                                    kind: "primary"
                                    iconName: "view-refresh"
                                    text: qsTr("Open wheel")
                                    onClicked: App.showWheel(page.sel.files, page.mode)
                                }
                            }
                        }

                        FileGarden {
                            Layout.preferredWidth: implicitWidth
                            Layout.preferredHeight: implicitHeight
                            Layout.alignment: Qt.AlignVCenter
                            targeted: dropArea.containsDrag
                            selected: page.hasFiles
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
                        KeyChip { keyText: page.keyName(App.inputd.actionsKey, "alt"); label: qsTr("File actions") }
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

            // "Download a link": the whole card is the button (upstream CardButton),
            // so hover and keyboard focus outline the card itself in gold.
            AbstractButton {
                id: linkCard
                objectName: "downloadLinkButton"
                Layout.fillWidth: true
                Layout.leftMargin: 28
                Layout.rightMargin: 28
                Layout.topMargin: -10
                padding: linkCard.visualFocus ? 17 : 18
                leftPadding: linkCard.visualFocus ? 21 : 22
                rightPadding: leftPadding
                hoverEnabled: true
                focusPolicy: Qt.StrongFocus
                Accessible.role: Accessible.Button
                Accessible.name: qsTr("Download a link")
                Accessible.description: qsTr("Video, audio, or an image. Straight into your tools.")
                ToolTip.visible: hovered
                ToolTip.delay: 600
                ToolTip.text: qsTr("Save video, audio or a file from an http(s) link")
                onClicked: App.showDownloadFromClipboard()

                contentItem: RowLayout {
                    spacing: 14
                    Text {
                        text: "↓"
                        font.pixelSize: 28
                        color: Theme.dark ? Theme.gold : Theme.goldInk
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Text {
                            Layout.fillWidth: true
                            text: qsTr("Download a link")
                            elide: Text.ElideRight
                            font.pixelSize: Theme.sizeHeading - 1
                            font.weight: Font.DemiBold
                            color: Theme.text
                        }
                        Text {
                            Layout.fillWidth: true
                            text: qsTr("Video, audio, or an image. Straight into your tools.")
                            elide: Text.ElideRight
                            font.pixelSize: Theme.sizeSmall + 1
                            color: Theme.secondaryText
                        }
                    }
                    Text {
                        text: "Ctrl+Shift+D"
                        font.pixelSize: Theme.sizeSmall + 1
                        color: Theme.secondaryText
                    }
                }
                background: Rectangle {
                    radius: Theme.radiusCard
                    color: linkCard.down ? Theme.sage : Theme.surface
                    border.width: linkCard.visualFocus ? 2 : 1
                    border.color: linkCard.hovered || linkCard.visualFocus ? Theme.gold : Theme.border
                    Behavior on border.color { ColorAnimation { duration: Theme.quick } }
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

            // Freshly made: the three newest Library files, with a quick search.
            RowLayout {
                Layout.fillWidth: true
                Layout.leftMargin: 28
                Layout.rightMargin: 28
                Layout.topMargin: 8
                spacing: 8
                Text {
                    text: qsTr("Freshly made")
                    font.pixelSize: Theme.sizeHeading + 3
                    font.weight: Font.DemiBold
                    color: Theme.text
                }
                Text {
                    Layout.alignment: Qt.AlignVCenter
                    Layout.leftMargin: 2
                    text: page.recentTotal
                    font.pixelSize: Theme.sizeSmall
                    color: Theme.secondaryText
                }
                Item { Layout.fillWidth: true }
                TextField {
                    id: recentSearch
                    objectName: "homeRecentSearch"
                    Layout.preferredWidth: 180
                    visible: page.recentTotal > 0
                    placeholderText: qsTr("Find a recent file")
                    selectByMouse: true
                    leftPadding: 11
                    rightPadding: 11
                    topPadding: 7
                    bottomPadding: 7
                    color: Theme.text
                    placeholderTextColor: Theme.tertiaryText
                    font.pixelSize: Theme.sizeSmall + 1
                    background: Rectangle {
                        radius: Theme.radiusCompact
                        color: Theme.inset
                        border.width: recentSearch.activeFocus ? 2 : 1
                        border.color: recentSearch.activeFocus ? Theme.gold : Theme.line
                    }
                }
                NButton {
                    objectName: "homeSeeAllButton"
                    compact: true
                    text: qsTr("See all  ↗")
                    onClicked: page.libraryRequested()
                }
            }

            Card {
                Layout.fillWidth: true
                Layout.leftMargin: 28
                Layout.rightMargin: 28
                Layout.topMargin: -12
                implicitHeight: recentColumn.implicitHeight + 14

                ColumnLayout {
                    id: recentColumn
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 7
                    spacing: 2

                    Text {
                        Layout.fillWidth: true
                        Layout.margins: 15
                        visible: page.recentHits.length === 0
                        wrapMode: Text.Wrap
                        font.pixelSize: Theme.sizeBody
                        color: Theme.secondaryText
                        text: page.recentTotal === 0 ? qsTr("Your next fresh copy will land here.")
                                                     : qsTr("No matches. Try a file name or folder.")
                    }

                    Repeater {
                        model: page.recentHits.slice(0, 3)
                        delegate: Rectangle {
                            id: recentRow
                            required property string modelData
                            required property int index
                            Layout.fillWidth: true
                            implicitHeight: recentRowLayout.implicitHeight + 16
                            radius: Theme.radiusRow
                            color: rowHover.hovered ? Theme.hover : "transparent"

                            HoverHandler { id: rowHover }
                            TapHandler { onDoubleTapped: App.actions.open(recentRow.modelData) }

                            RowLayout {
                                id: recentRowLayout
                                anchors.fill: parent
                                anchors.leftMargin: 10
                                anchors.rightMargin: 6
                                spacing: 12
                                Rectangle {
                                    Layout.preferredWidth: 34
                                    Layout.preferredHeight: 34
                                    radius: 10
                                    color: Theme.inset
                                    Image {
                                        anchors.centerIn: parent
                                        source: "image://icon/" + App.mimeIcon(recentRow.modelData)
                                        sourceSize: Qt.size(48, 48)
                                        width: 22
                                        height: 22
                                    }
                                }
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 1
                                    Text {
                                        Layout.fillWidth: true
                                        elide: Text.ElideMiddle
                                        text: App.fileName(recentRow.modelData)
                                        font.pixelSize: Theme.sizeBody
                                        font.weight: Font.DemiBold
                                        color: Theme.text
                                    }
                                    Text {
                                        Layout.fillWidth: true
                                        elide: Text.ElideMiddle
                                        text: recentRow.modelData.substring(0, recentRow.modelData.lastIndexOf("/"))
                                        font.pixelSize: Theme.sizeSmall
                                        color: Theme.secondaryText
                                    }
                                }
                                NButton {
                                    objectName: "homeRecentPreview_" + recentRow.index
                                    compact: true
                                    kind: "quiet"
                                    iconName: "view-preview"
                                    Accessible.name: qsTr("Preview")
                                    ToolTip.visible: hovered
                                    ToolTip.text: qsTr("Preview")
                                    onClicked: App.showPreview(recentRow.modelData, page.recentHits)
                                }
                                NButton {
                                    compact: true
                                    kind: "quiet"
                                    iconName: "view-refresh"
                                    Accessible.name: qsTr("Use again")
                                    ToolTip.visible: hovered
                                    ToolTip.text: qsTr("Use again")
                                    onClicked: App.reuseFiles([recentRow.modelData])
                                }
                                NButton {
                                    compact: true
                                    kind: "quiet"
                                    iconName: "document-open"
                                    Accessible.name: qsTr("Open")
                                    ToolTip.visible: hovered
                                    ToolTip.text: qsTr("Open")
                                    onClicked: App.actions.open(recentRow.modelData)
                                }
                                NButton {
                                    compact: true
                                    kind: "quiet"
                                    iconName: "folder-open"
                                    Accessible.name: qsTr("Show in folder")
                                    ToolTip.visible: hovered
                                    ToolTip.text: qsTr("Show in folder")
                                    onClicked: App.actions.reveal([recentRow.modelData])
                                }
                            }
                        }
                    }
                }
            }

            Expander {
                objectName: "toolsExpander"
                Layout.fillWidth: true
                Layout.leftMargin: 32
                Layout.rightMargin: 32
                Layout.topMargin: -6
                title: qsTr("Tools & shortcuts")

                Flow {
                    Layout.fillWidth: true
                    spacing: 8
                    NButton {
                        compact: true
                        text: App.deps.missingCount > 0 ? qsTr("Set up tools (%1 missing)").arg(App.deps.missingCount)
                                                        : qsTr("Check tools")
                        onClicked: page.toolsRequested()
                    }
                    NButton {
                        compact: true
                        text: qsTr("Features & shortcuts")
                        onClicked: page.shortcutsRequested()
                    }
                }
            }

            Item { Layout.preferredHeight: 8 }
        }
    }
}
