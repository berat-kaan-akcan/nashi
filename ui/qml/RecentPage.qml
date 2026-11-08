pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

// Library: files made with Nashi. Searching or removing never touches files.
Page {
    id: page
    background: null
    padding: 0

    // Ctrl+F / Ctrl+2 in the Home window.
    // The page may only become visible after the tab switch is laid out.
    property bool searchFocusPending: false
    function focusSearch() {
        searchFocusPending = !search.visible
        if (search.visible) {
            search.forceActiveFocus()
            search.selectAll()
        }
    }
    onVisibleChanged: if (visible && searchFocusPending) focusSearch()

    function matches(name, folder) {
        return App.matchesSearch(search.text, name, folder)
    }

    property int revision: 0
    Connections {
        target: App.recent
        function onRowsInserted() { page.revision++ }
        function onRowsRemoved() { page.revision++ }
        function onModelReset() { page.revision++ }
        function onDataChanged() { page.revision++ }
    }
    readonly property int matchCount: {
        page.revision
        let n = 0
        for (let i = 0; i < list.count; ++i) {
            const item = list.itemAtIndex(i)
            if (!item || item.shown)
                ++n
        }
        return n
    }

    function previewPath(path) {
        App.showPreview(path, App.filteredRecentPaths(search.text))
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.leftMargin: 28
        anchors.rightMargin: 28
        anchors.bottomMargin: 8
        spacing: 14

        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: 8
            PageHeading {
                Layout.fillWidth: true
                title: qsTr("Your Library")
                subtitle: qsTr("Freshly made, pasted, and ready for another round.")
            }
            NButton {
                Layout.alignment: Qt.AlignBottom
                compact: true
                kind: "quiet"
                iconName: "edit-clear-history"
                text: qsTr("Clear list")
                enabled: App.recent.count > 0
                onClicked: App.recent.clear()
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.topMargin: 6
            visible: App.recent.count > 0
            spacing: 4
            Text {
                Layout.leftMargin: 4
                text: qsTr("Find a file · Ctrl+F")
                font.pixelSize: Theme.sizeSmall + 1
                color: Theme.secondaryText
            }
            NTextField {
                id: search
                objectName: "librarySearch"
                Layout.fillWidth: true
                placeholderText: qsTr("File or folder name")
                Accessible.name: qsTr("Find a file")
                Keys.onDownPressed: list.forceActiveFocus()
            }
        }

        Card {
            Layout.fillWidth: true
            visible: App.recent.count === 0 || (search.text.length > 0 && page.matchCount === 0)
            implicitHeight: emptyText.implicitHeight + 44
            Text {
                id: emptyText
                anchors.fill: parent
                anchors.margins: 22
                wrapMode: Text.Wrap
                verticalAlignment: Text.AlignVCenter
                font.pixelSize: Theme.sizeBody + 1
                color: Theme.secondaryText
                text: App.recent.count === 0 ? qsTr("Nothing here yet. Paste something or make your first fresh copy.")
                                             : qsTr("No matches. Try a file name or folder.")
            }
        }
        Item {
            Layout.fillHeight: true
            visible: App.recent.count === 0
        }

        ListView {
            id: list
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: App.recent.count > 0
            clip: true
            model: App.recent
            spacing: 6
            keyNavigationEnabled: true
            ScrollBar.vertical: ScrollBar {}

            delegate: Item {
                id: row
                required property int index
                required property string path
                required property string name
                required property string folder
                required property string when
                required property bool exists
                readonly property bool shown: page.matches(name, folder)
                width: ListView.view.width
                height: shown ? card.implicitHeight : 0
                visible: shown

                Rectangle {
                    id: card
                    anchors.fill: parent
                    implicitHeight: rowLayout.implicitHeight + 20
                    radius: Theme.radiusCard
                    color: Theme.surface
                    border.width: row.ListView.isCurrentItem && list.activeFocus ? 2 : 1
                    border.color: row.ListView.isCurrentItem && list.activeFocus ? Theme.gold
                                  : rowHover.hovered ? Theme.line : Theme.border

                    HoverHandler { id: rowHover }

                    TapHandler {
                        onTapped: {
                            list.currentIndex = row.index
                            list.forceActiveFocus()
                        }
                        onDoubleTapped: if (row.exists) App.actions.open(row.path)
                    }

                    RowLayout {
                        id: rowLayout
                        anchors.fill: parent
                        anchors.leftMargin: 14
                        anchors.rightMargin: 10
                        spacing: 12

                        Image {
                            Layout.preferredWidth: 29
                            Layout.preferredHeight: 29
                            source: "image://icon/" + App.mimeIcon(row.path)
                            sourceSize: Qt.size(58, 58)
                            opacity: row.exists ? 1 : 0.4
                        }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 2
                            Text {
                                Layout.fillWidth: true
                                elide: Text.ElideMiddle
                                text: row.name
                                font.pixelSize: Theme.sizeBody
                                font.weight: Font.DemiBold
                                font.strikeout: !row.exists
                                color: Theme.text
                            }
                            Text {
                                Layout.fillWidth: true
                                elide: Text.ElideMiddle
                                font.pixelSize: Theme.sizeSmall
                                color: row.exists ? Theme.secondaryText : Theme.danger
                                text: row.exists ? row.when + "  ·  " + row.folder
                                                 : row.when + "  ·  " + qsTr("Files missing")
                            }
                        }
                        NButton {
                            objectName: "recentPreviewButton_" + row.index
                            compact: true
                            kind: "quiet"
                            iconName: "view-preview"
                            Accessible.name: qsTr("Preview")
                            enabled: row.exists
                            onClicked: page.previewPath(row.path)
                            ToolTip.visible: hovered
                            ToolTip.text: qsTr("Preview · Space")
                        }
                        NButton {
                            objectName: "recentReuseButton_" + row.index
                            compact: true
                            kind: "quiet"
                            iconName: "view-refresh"
                            Accessible.name: qsTr("Use again")
                            enabled: row.exists
                            onClicked: App.reuseFiles([row.path])
                            ToolTip.visible: hovered
                            ToolTip.text: qsTr("Use again")
                        }
                        NButton {
                            objectName: "recentFloatButton_" + row.index
                            compact: true
                            kind: "quiet"
                            iconName: "pin"
                            Accessible.name: qsTr("Add to Float")
                            enabled: row.exists
                            onClicked: App.addToShelf([row.path])
                            ToolTip.visible: hovered
                            ToolTip.text: qsTr("Add to Float")
                        }
                        NButton {
                            objectName: "recentShareButton_" + row.index
                            visible: App.shareAvailable
                            compact: true
                            kind: "quiet"
                            iconName: "document-send"
                            Accessible.name: qsTr("Share")
                            enabled: row.exists
                            onClicked: App.showShare([row.path])
                            ToolTip.visible: hovered
                            ToolTip.text: qsTr("Share")
                        }
                        NButton {
                            compact: true
                            kind: "quiet"
                            iconName: "document-open"
                            Accessible.name: qsTr("Open")
                            enabled: row.exists
                            onClicked: App.actions.open(row.path)
                            ToolTip.visible: hovered
                            ToolTip.text: qsTr("Open")
                        }
                        NButton {
                            compact: true
                            kind: "quiet"
                            iconName: "folder-open"
                            Accessible.name: qsTr("Show in folder")
                            enabled: row.exists
                            onClicked: App.actions.reveal([row.path])
                            ToolTip.visible: hovered
                            ToolTip.text: qsTr("Show in folder")
                        }
                        NButton {
                            compact: true
                            kind: "quiet"
                            iconName: "list-remove"
                            Accessible.name: qsTr("Remove from list")
                            onClicked: App.recent.remove(row.index)
                            ToolTip.visible: hovered
                            ToolTip.text: qsTr("Remove from list")
                        }
                    }
                }
            }

            Keys.onSpacePressed: {
                if (currentItem && currentItem.exists)
                    page.previewPath(currentItem.path)
            }
            Keys.onReturnPressed: {
                if (currentItem && currentItem.exists)
                    App.actions.open(currentItem.path)
            }
        }

        Text {
            Layout.fillWidth: true
            visible: App.recent.count > 0
            text: qsTr("↑/↓ choose · Space preview · Enter open · double-click open")
            font.pixelSize: Theme.sizeTiny
            color: Theme.secondaryText
        }
    }
}
