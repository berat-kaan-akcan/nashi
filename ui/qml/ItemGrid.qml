pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import NashiApp

// Wheel items as a clickable grid (Home window).
GridView {
    id: grid

    property WheelModel wheelModel: null
    signal picked(string kind, string itemId, string title)

    clip: true
    cellWidth: 150
    cellHeight: 92
    model: wheelModel
    boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: ScrollBar {}

    delegate: Item {
        id: cell
        required property string itemId
        required property string kind
        required property string title
        required property string iconName
        required property bool available
        required property string missing
        width: grid.cellWidth
        height: grid.cellHeight

        Button {
            anchors.fill: parent
            anchors.margins: 4
            enabled: cell.available
            display: AbstractButton.TextUnderIcon
            icon.source: cell.iconName.length > 0 ? "image://icon/" + cell.iconName : ""
            icon.width: 28
            icon.height: 28
            text: cell.available ? cell.title : qsTr("%1\n(needs %2)").arg(cell.title).arg(cell.missing)
            onClicked: grid.picked(cell.kind, cell.itemId, cell.title)
            ToolTip.visible: hovered && !cell.available
            ToolTip.text: qsTr("Missing tool: %1").arg(cell.missing)
            ToolTip.delay: 300
        }
    }
}
