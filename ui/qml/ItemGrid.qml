pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import NashiApp

// Wheel items as a grid of small cards (Home window).
GridView {
    id: grid

    property WheelModel wheelModel: null
    signal picked(string kind, string itemId, string title)

    clip: true
    cellWidth: Math.max(140, width / Math.max(1, Math.floor(width / 150)))
    cellHeight: 96
    model: wheelModel
    boundsBehavior: Flickable.StopAtBounds

    delegate: Item {
        id: cell
        required property int index
        required property string itemId
        required property string kind
        required property string title
        required property string iconName
        required property bool available
        required property string missing
        width: grid.cellWidth
        height: grid.cellHeight

        AbstractButton {
            id: button
            anchors.fill: parent
            anchors.margins: 6
            enabled: cell.available
            hoverEnabled: true
            focusPolicy: Qt.StrongFocus
            Accessible.role: Accessible.Button
            Accessible.name: cell.title
            onClicked: grid.picked(cell.kind, cell.itemId, cell.title)
            ToolTip.visible: hovered && !cell.available
            ToolTip.text: qsTr("Missing tool: %1").arg(cell.missing)
            ToolTip.delay: 300
            scale: Theme.motion && down ? 0.97 : 1
            Behavior on scale { NumberAnimation { duration: Theme.quick } }

            background: Rectangle {
                radius: Theme.radiusRow
                color: button.hovered ? Qt.rgba(Theme.gold.r, Theme.gold.g, Theme.gold.b, 0.16) : Theme.surface
                border.width: 1
                border.color: button.hovered ? Theme.gold : Theme.border
                Behavior on color { ColorAnimation { duration: Theme.quick } }
                Rectangle {
                    anchors.fill: parent
                    anchors.margins: -3
                    radius: parent.radius + 3
                    color: "transparent"
                    border.width: 2
                    border.color: Theme.goldInk
                    visible: button.visualFocus
                }
            }

            contentItem: Item {
              Column {
                anchors.centerIn: parent
                width: parent.width
                spacing: 6
                opacity: cell.available ? 1 : 0.5
                Image {
                    anchors.horizontalCenter: parent.horizontalCenter
                    visible: cell.iconName.length > 0
                    source: visible ? "image://icon/" + cell.iconName : ""
                    sourceSize: Qt.size(48, 48)
                    width: 24
                    height: 24
                }
                Text {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                    maximumLineCount: 2
                    elide: Text.ElideRight
                    text: cell.kind === "convert" ? cell.title.toUpperCase() : cell.title
                    font.pixelSize: cell.kind === "convert" ? Theme.sizeBody : Theme.sizeSmall + 1
                    font.weight: Font.DemiBold
                    font.letterSpacing: cell.kind === "convert" ? 0.6 : 0
                    color: Theme.text
                }
                Text {
                    width: parent.width
                    visible: !cell.available
                    horizontalAlignment: Text.AlignHCenter
                    elide: Text.ElideRight
                    text: qsTr("needs %1").arg(cell.missing)
                    font.pixelSize: Theme.sizeTiny
                    color: Theme.secondaryText
                }
              }
            }
        }
    }
}
