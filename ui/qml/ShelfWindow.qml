pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import NashiApp

// Floating holding shelf ("Float") docked near the right screen edge.
// Accepts drops as CopyAction and lets files be dragged back out as CopyAction.
Window {
    id: root

    visible: false
    width: 272
    height: Math.max(168, Math.min(content.implicitHeight + 24, 460))
    color: "transparent"
    title: qsTr("Nashi Float")
    onHeightChanged: App.shelfResized()

    Rectangle {
        id: frame
        anchors.fill: parent
        radius: 20
        color: Theme.surface
        border.width: shelfDrop.containsDrag ? 2 : 1
        border.color: shelfDrop.containsDrag ? Theme.gold : Theme.border

        ColumnLayout {
            id: content
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: 12
            spacing: 10

            // Header bar (also draggable on non-layer-shell desktops)
            RowLayout {
                Layout.fillWidth: true
                spacing: 7

                Item {
                    Layout.fillWidth: true
                    implicitHeight: titleRow.implicitHeight

                    DragHandler {
                        target: null
                        onActiveChanged: {
                            if (active && !App.layerShell)
                                root.startSystemMove()
                        }
                    }

                    RowLayout {
                        id: titleRow
                        anchors.fill: parent
                        spacing: 7

                        NashiMark {
                            size: 22
                            happy: App.shelf.count > 0 || shelfDrop.containsDrag
                        }
                        Text {
                            text: qsTr("Float")
                            font.pixelSize: Theme.sizeBody + 1
                            font.weight: Font.Bold
                            color: Theme.text
                        }
                        Pill {
                            objectName: "shelfCountPill"
                            visible: App.shelf.count > 0
                            text: String(App.shelf.count)
                            tint: Theme.gold
                            textColor: Theme.goldInk
                        }
                        Item { Layout.fillWidth: true }
                    }
                }

                NButton {
                    objectName: "shelfClearButton"
                    visible: App.shelf.count > 0
                    compact: true
                    kind: "quiet"
                    text: qsTr("Clear")
                    onClicked: App.shelf.clear()
                }
                NButton {
                    objectName: "shelfCloseButton"
                    compact: true
                    kind: "quiet"
                    leftPadding: 6
                    rightPadding: 6
                    iconName: "window-close"
                    Accessible.name: qsTr("Hide Float Shelf")
                    ToolTip.visible: hovered
                    ToolTip.text: qsTr("Hide Float Shelf")
                    onClicked: App.shelf.hide()
                }
            }

            // Empty drop well
            Rectangle {
                objectName: "shelfEmptyWell"
                Layout.fillWidth: true
                Layout.preferredHeight: 112
                visible: App.shelf.count === 0
                radius: Theme.radiusRow
                color: shelfDrop.containsDrag ? Theme.sand : Theme.inset
                border.width: 1
                border.color: shelfDrop.containsDrag ? Theme.gold : Theme.border

                ColumnLayout {
                    anchors.centerIn: parent
                    width: parent.width - 28
                    spacing: 6

                    Image {
                        Layout.alignment: Qt.AlignHCenter
                        source: "image://icon/go-down"
                        sourceSize: Qt.size(40, 40)
                        width: 20
                        height: 20
                        opacity: 0.75
                    }
                    Text {
                        Layout.fillWidth: true
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                        font.pixelSize: Theme.sizeSmall + 1
                        font.weight: Font.DemiBold
                        color: Theme.text
                        text: shelfDrop.containsDrag ? qsTr("Drop to hold here")
                                                     : qsTr("Drop files here to hold them")
                    }
                    Text {
                        Layout.fillWidth: true
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                        font.pixelSize: Theme.sizeTiny
                        color: Theme.secondaryText
                        text: qsTr("Drag back out into any app. Originals never move.")
                    }
                }
            }

            // Held file tiles
            ListView {
                id: shelfList
                objectName: "shelfListView"
                Layout.fillWidth: true
                Layout.preferredHeight: Math.min(contentHeight, 304)
                visible: App.shelf.count > 0
                clip: true
                spacing: 6
                model: App.shelf
                ScrollBar.vertical: ScrollBar {}

                delegate: Rectangle {
                    id: tile
                    required property int index
                    required property string path
                    required property string name
                    required property string icon
                    required property bool exists

                    width: ListView.view.width
                    height: 44
                    radius: 12
                    color: gripHover.hovered ? Theme.inset : Theme.canvas
                    border.width: 1
                    border.color: gripDrag.active ? Theme.gold : Theme.border

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 9
                        anchors.rightMargin: 6
                        spacing: 4

                        // Draggable + clickable file grip
                        Item {
                            id: grip
                            objectName: "shelfDragSource"
                            Layout.fillWidth: true
                            Layout.fillHeight: true

                            Drag.active: gripDrag.active
                            Drag.dragType: Drag.Automatic
                            Drag.supportedActions: Qt.CopyAction
                            Drag.mimeData: ({
                                "text/uri-list": App.shelf.uriListFor([tile.path]),
                                "text/plain": tile.path
                            })

                            HoverHandler { id: gripHover }
                            TapHandler {
                                onTapped: {
                                    if (tile.exists)
                                        App.actions.open(tile.path)
                                }
                            }
                            DragHandler {
                                id: gripDrag
                                target: null
                                enabled: tile.exists
                            }

                            ToolTip.visible: gripHover.hovered && !gripDrag.active
                            ToolTip.delay: 500
                            ToolTip.text: tile.exists ? qsTr("Drag out or click to open\n%1").arg(tile.path)
                                                      : qsTr("File no longer exists")

                            RowLayout {
                                anchors.fill: parent
                                spacing: 8

                                Rectangle {
                                    Layout.preferredWidth: 28
                                    Layout.preferredHeight: 28
                                    radius: 8
                                    color: Theme.surface
                                    Image {
                                        anchors.centerIn: parent
                                        source: "image://icon/" + tile.icon
                                        sourceSize: Qt.size(36, 36)
                                        width: 18
                                        height: 18
                                        opacity: tile.exists ? 1 : 0.4
                                    }
                                }

                                Text {
                                    Layout.fillWidth: true
                                    elide: Text.ElideMiddle
                                    text: tile.name
                                    font.pixelSize: Theme.sizeSmall + 1
                                    font.weight: Font.Medium
                                    font.strikeout: !tile.exists
                                    color: Theme.text
                                }
                            }
                        }

                        NButton {
                            compact: true
                            kind: "quiet"
                            leftPadding: 5
                            rightPadding: 5
                            topPadding: 4
                            bottomPadding: 4
                            iconName: "view-refresh"
                            enabled: tile.exists
                            Accessible.name: qsTr("Use in Nashi")
                            ToolTip.visible: hovered
                            ToolTip.text: qsTr("Use in Nashi")
                            onClicked: App.reuseFiles([tile.path])
                        }
                        NButton {
                            visible: App.shareAvailable
                            compact: true
                            kind: "quiet"
                            leftPadding: 5
                            rightPadding: 5
                            topPadding: 4
                            bottomPadding: 4
                            iconName: "document-send"
                            enabled: tile.exists
                            Accessible.name: qsTr("Share")
                            ToolTip.visible: hovered
                            ToolTip.text: qsTr("Share")
                            onClicked: App.showShare([tile.path])
                        }
                        NButton {
                            compact: true
                            kind: "quiet"
                            leftPadding: 5
                            rightPadding: 5
                            topPadding: 4
                            bottomPadding: 4
                            iconName: "window-close"
                            Accessible.name: qsTr("Remove %1").arg(tile.name)
                            ToolTip.visible: hovered
                            ToolTip.text: qsTr("Remove")
                            onClicked: App.shelf.remove(tile.index)
                        }
                    }
                }
            }

            // Bottom batch actions: "Drag all" + "Use in Nashi" + "Share"
            RowLayout {
                Layout.fillWidth: true
                visible: App.shelf.count > 0
                spacing: 6

                Rectangle {
                    id: dragAllChip
                    objectName: "shelfDragSource"
                    Layout.fillWidth: true
                    implicitHeight: 30
                    radius: Theme.radiusCompact
                    color: dragAllHover.hovered ? Theme.sand : Theme.inset
                    border.width: 1
                    border.color: dragAllHandler.active ? Theme.gold : Theme.border

                    Drag.active: dragAllHandler.active
                    Drag.dragType: Drag.Automatic
                    Drag.supportedActions: Qt.CopyAction
                    Drag.mimeData: ({
                        "text/uri-list": App.shelf.uriListFor(App.shelf.files),
                        "text/plain": App.shelf.files.join("\n")
                    })

                    HoverHandler { id: dragAllHover }
                    TapHandler { onTapped: App.actions.copy(App.shelf.files) }
                    DragHandler {
                        id: dragAllHandler
                        target: null
                    }

                    ToolTip.visible: dragAllHover.hovered && !dragAllHandler.active
                    ToolTip.delay: 400
                    ToolTip.text: qsTr("Drag all files out, or click to copy")

                    Row {
                        anchors.centerIn: parent
                        spacing: 5
                        Image {
                            anchors.verticalCenter: parent.verticalCenter
                            source: "image://icon/edit-copy"
                            sourceSize: Qt.size(28, 28)
                            width: 13
                            height: 13
                        }
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: qsTr("Drag all")
                            font.pixelSize: Theme.sizeSmall
                            font.weight: Font.DemiBold
                            color: Theme.text
                        }
                    }
                }

                NButton {
                    objectName: "shelfUseAllButton"
                    compact: true
                    kind: "primary"
                    iconName: "view-refresh"
                    text: qsTr("Use in Nashi")
                    onClicked: App.reuseFiles(App.shelf.files)
                }

                NButton {
                    objectName: "shelfShareAllButton"
                    visible: App.shareAvailable
                    compact: true
                    kind: "secondary"
                    leftPadding: 8
                    rightPadding: 8
                    iconName: "document-send"
                    Accessible.name: qsTr("Share all")
                    ToolTip.visible: hovered
                    ToolTip.text: qsTr("Share all")
                    onClicked: App.showShare(App.shelf.files)
                }
            }
        }

        DropArea {
            id: shelfDrop
            objectName: "shelfDropArea"
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
                if (drop.source && drop.source.objectName === "shelfDragSource")
                    return
                if (drop.hasUrls)
                    App.shelf.addUrls(drop.urls)
            }
        }
    }
}
