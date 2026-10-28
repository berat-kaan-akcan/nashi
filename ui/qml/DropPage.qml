import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import NashiApp

// Drop zone + the wheel's items as a grid.
Page {
    id: page

    readonly property Selection sel: App.home

    function modeIndex(m) { return m === "tools" ? 1 : m === "actions" ? 2 : 0 }

    padding: 12

    FileDialog {
        id: fileDialog
        fileMode: FileDialog.OpenFiles
        title: qsTr("Choose files")
        onAccepted: page.sel.setUrls(selectedFiles)
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 10

        Rectangle {
            id: zone
            Layout.fillWidth: true
            Layout.preferredHeight: page.sel.fileCount > 0 ? 84 : 220
            radius: 10
            color: dropArea.containsDrag ? Qt.rgba(palette.highlight.r, palette.highlight.g, palette.highlight.b, 0.18)
                                         : palette.base
            border.color: dropArea.containsDrag ? palette.highlight : palette.mid
            border.width: 2
            Behavior on Layout.preferredHeight { NumberAnimation { duration: 120 } }

            ColumnLayout {
                anchors.centerIn: parent
                width: parent.width - 32
                spacing: 6
                Label {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    font.pixelSize: 16
                    elide: Text.ElideMiddle
                    text: page.sel.fileCount > 0 ? page.sel.summary : qsTr("Drop files here")
                }
                Label {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    opacity: 0.7
                    visible: page.sel.fileCount === 0
                    wrapMode: Text.Wrap
                    text: qsTr("Files are never moved or changed; results are saved next to them.")
                }
                Button {
                    Layout.alignment: Qt.AlignHCenter
                    text: page.sel.fileCount > 0 ? qsTr("Choose other files…") : qsTr("Choose files…")
                    icon.name: "document-open"
                    onClicked: fileDialog.open()
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

        RowLayout {
            Layout.fillWidth: true
            visible: page.sel.fileCount > 0
            TabBar {
                id: modeBar
                Layout.fillWidth: true
                currentIndex: page.modeIndex(page.sel.model.mode)
                onCurrentIndexChanged: page.sel.model.mode = ["convert", "tools", "actions"][currentIndex]
                TabButton { text: qsTr("Convert") }
                TabButton { text: qsTr("Tools") }
                TabButton { text: qsTr("Actions") }
            }
            ToolButton {
                icon.name: "edit-clear"
                text: qsTr("Clear")
                onClicked: page.sel.clear()
            }
        }

        BusyIndicator {
            Layout.alignment: Qt.AlignHCenter
            visible: running
            running: page.sel.loading
        }

        Label {
            Layout.fillWidth: true
            visible: page.sel.error.length > 0
            wrapMode: Text.Wrap
            color: "#da4453"
            text: qsTr("Could not read the file types: %1").arg(page.sel.error)
        }

        Label {
            Layout.fillWidth: true
            visible: page.sel.ready && page.sel.model.count === 0
            horizontalAlignment: Text.AlignHCenter
            opacity: 0.7
            text: qsTr("Nothing in this group for these files.")
        }

        ItemGrid {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: page.sel.ready
            wheelModel: page.sel.model
            onPicked: (kind, itemId, title) => App.runItem(kind, itemId, title, page.sel.files)
        }

        Item {
            Layout.fillHeight: true
            visible: !page.sel.ready
        }
    }
}
