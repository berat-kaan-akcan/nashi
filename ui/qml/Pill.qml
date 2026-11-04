import QtQuick
import NashiApp

// Small capsule label, e.g. "START HERE".
Rectangle {
    id: pill
    property string text: ""
    property string iconName: ""
    property color tint: Theme.forest
    property color textColor: tint

    implicitWidth: row.implicitWidth + 18
    implicitHeight: row.implicitHeight + 10
    radius: height / 2
    color: Qt.rgba(tint.r, tint.g, tint.b, 0.11)

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 5
        Image {
            anchors.verticalCenter: parent.verticalCenter
            visible: pill.iconName.length > 0
            source: visible ? "image://icon/" + pill.iconName : ""
            sourceSize: Qt.size(24, 24)
            width: 12
            height: 12
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: pill.text
            font.pixelSize: Theme.sizeTiny
            font.weight: Font.DemiBold
            color: pill.textColor
        }
    }
}
