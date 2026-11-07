import QtQuick
import QtQuick.Controls
import NashiApp

// Nashi button: "primary" (gold gradient), "secondary" (card) or "quiet".
AbstractButton {
    id: control

    property string kind: "secondary"
    property bool compact: false
    // Current page in a switcher: sage fill, as upstream Windows marks it.
    property bool selected: false
    property string iconName: ""
    readonly property bool primary: kind === "primary"
    readonly property int r: compact ? Theme.radiusCompact : Theme.radiusButton

    implicitWidth: Math.max(compact ? 28 : 36, row.implicitWidth + leftPadding + rightPadding)
    implicitHeight: row.implicitHeight + topPadding + bottomPadding
    leftPadding: compact ? 10 : 15
    rightPadding: leftPadding
    topPadding: compact ? 6 : 10
    bottomPadding: topPadding
    hoverEnabled: true
    focusPolicy: Qt.StrongFocus
    opacity: enabled ? 1 : 0.4
    scale: !Theme.motion ? 1 : down ? 0.97 : hovered && enabled ? 1.015 : 1
    Behavior on scale { NumberAnimation { duration: Theme.quick; easing.type: Easing.OutCubic } }

    Accessible.role: Accessible.Button
    Accessible.name: text
    ToolTip.delay: 400

    contentItem: Item {
        implicitWidth: row.implicitWidth
        implicitHeight: row.implicitHeight
        Row {
            id: row
            anchors.centerIn: parent
            spacing: 6
            Image {
                anchors.verticalCenter: parent.verticalCenter
                visible: control.iconName.length > 0
                source: visible ? "image://icon/" + control.iconName : ""
                sourceSize: Qt.size(width * 2, height * 2)
                width: control.compact ? 14 : 16
                height: width
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: control.text.length > 0
                text: control.text
                font.pixelSize: control.compact ? Theme.sizeSmall : Theme.sizeBody
                font.weight: Font.DemiBold
                color: control.primary ? Theme.buttonInk : Theme.text
            }
        }
    }

    background: Rectangle {
        radius: control.r
        color: control.selected ? Theme.sage : control.kind === "quiet" ? "transparent" : Theme.surface
        border.width: control.kind === "quiet" ? 0 : 1
        border.color: control.primary ? Qt.rgba(1, 1, 1, 0.12) : Theme.border
        gradient: control.primary ? goldGradient : null
        Gradient {
            id: goldGradient
            orientation: Gradient.Horizontal
            GradientStop { position: 0; color: Theme.gold }
            GradientStop { position: 1; color: Theme.goldEnd }
        }
        Rectangle {
            anchors.fill: parent
            radius: parent.radius
            color: control.hovered && !control.primary ? Theme.hover
                   : control.hovered ? Qt.rgba(1, 1, 1, 0.12) : "transparent"
        }
        Rectangle {
            anchors.fill: parent
            anchors.margins: -3
            radius: parent.radius + 3
            color: "transparent"
            border.width: 2
            border.color: Theme.goldInk
            visible: control.visualFocus
        }
    }
}
