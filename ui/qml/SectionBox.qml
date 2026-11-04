import QtQuick
import QtQuick.Layouts
import NashiApp

// A titled card (upstream NashiGroupBoxStyle). Children go into a column.
Card {
    id: box
    property string title: ""
    default property alias content: column.data

    implicitHeight: column.implicitHeight + 40
    Layout.fillWidth: true

    ColumnLayout {
        id: column
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: 20
        spacing: 12

        Text {
            Layout.fillWidth: true
            visible: box.title.length > 0
            text: box.title
            font.pixelSize: Theme.sizeBody
            font.weight: Font.DemiBold
            color: Theme.forest
        }
    }
}
