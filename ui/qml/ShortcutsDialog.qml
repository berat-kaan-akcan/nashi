pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

// "Features & shortcuts" sheet, after upstream Windows ShowShortcuts. Lists
// only keys the Linux app really handles.
Dialog {
    id: sheet
    objectName: "shortcutsDialog"
    modal: true
    title: qsTr("Nashi · Features & shortcuts")
    anchors.centerIn: parent
    width: Math.min(560, parent ? parent.width - 56 : 560)
    padding: 22

    background: Rectangle {
        radius: Theme.radiusCard
        color: Theme.surface
        border.width: 1
        border.color: Theme.border
    }
    header: Text {
        text: sheet.title
        padding: 22
        bottomPadding: 4
        font.pixelSize: Theme.sizeHeading + 2
        font.weight: Font.Bold
        color: Theme.text
    }

    footer: Item {
        implicitHeight: closeButton.implicitHeight + 36
        NButton {
            id: closeButton
            anchors.right: parent.right
            anchors.rightMargin: 22
            anchors.verticalCenter: parent.verticalCenter
            kind: "primary"
            text: qsTr("Close")
            onClicked: sheet.close()
        }
    }

    readonly property var groups: [
        { title: qsTr("Home"), rows: [
            ["Ctrl+O", qsTr("Choose files")],
            ["Ctrl+V", qsTr("Paste files, text, an image or a link")],
            ["Ctrl+Shift+D", qsTr("Download a copied link")],
            ["Ctrl+1 / 2 / 3", qsTr("Home / Library / Tools")],
            ["Ctrl+,", qsTr("Settings")],
            ["Ctrl+F", qsTr("Find in Library")]
        ] },
        { title: qsTr("Drag a file"), rows: [
            [qsTr("Hold a key"), qsTr("By default Shift converts, Ctrl opens tools and Alt shows file actions")],
            [qsTr("Drop"), qsTr("Drop the file on a slice of the wheel to start")]
        ] },
        { title: qsTr("Wheel and Library"), rows: [
            ["← → ↑ ↓", qsTr("Choose a slice")],
            ["Enter", qsTr("Open the chosen slice or file")],
            ["Esc", qsTr("Close")],
            ["Space", qsTr("Preview the chosen Library file")]
        ] },
        { title: qsTr("Editors"), rows: [
            ["Space", qsTr("Play or pause")],
            ["← →", qsTr("Half a second back or forward (Shift: a tenth)")],
            ["I / O", qsTr("Set the start or end of the range")],
            ["M", qsTr("Mark a frame at the playhead")],
            ["Delete", qsTr("Remove the selected area")]
        ] }
    ]

    contentItem: ScrollView {
        id: scroll
        implicitHeight: Math.min(column.implicitHeight,
                                 Math.max(240, (sheet.parent ? sheet.parent.height : 700) - 220))
        contentWidth: availableWidth
        ColumnLayout {
            id: column
            width: scroll.availableWidth
            spacing: 14
            Repeater {
                model: sheet.groups
                delegate: ColumnLayout {
                    id: group
                    required property var modelData
                    Layout.fillWidth: true
                    spacing: 6
                    Text {
                        text: group.modelData.title
                        font.pixelSize: Theme.sizeSmall
                        font.weight: Font.DemiBold
                        font.capitalization: Font.AllUppercase
                        color: Theme.dark ? Theme.gold : Theme.goldInk
                    }
                    Repeater {
                        model: group.modelData.rows
                        delegate: RowLayout {
                            id: line
                            required property var modelData
                            Layout.fillWidth: true
                            spacing: 12
                            Text {
                                Layout.preferredWidth: 120
                                text: line.modelData[0]
                                font.pixelSize: Theme.sizeSmall + 1
                                font.weight: Font.DemiBold
                                color: Theme.text
                            }
                            Text {
                                Layout.fillWidth: true
                                text: line.modelData[1]
                                wrapMode: Text.Wrap
                                font.pixelSize: Theme.sizeSmall + 1
                                color: Theme.secondaryText
                            }
                        }
                    }
                }
            }
        }
    }
}
