import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Shapes
import NashiApp

// Collapsible section with a round chevron badge, after the upstream Windows
// Expander style: the badge and chevron turn gold when open or hovered.
ColumnLayout {
    id: expander
    property string title: ""
    property bool expanded: false
    default property alias content: body.data

    spacing: 0

    AbstractButton {
        id: header
        objectName: "expanderHeader"
        Layout.fillWidth: true
        padding: 6
        leftPadding: 2
        hoverEnabled: true
        focusPolicy: Qt.StrongFocus
        checkable: true
        checked: expander.expanded
        onToggled: expander.expanded = checked
        Accessible.role: Accessible.Button
        Accessible.name: expander.title
        readonly property bool lit: checked || hovered

        contentItem: Row {
            spacing: 8
            Rectangle {
                id: badge
                anchors.verticalCenter: parent.verticalCenter
                width: 20
                height: 20
                radius: 10
                color: header.lit ? Theme.surface : Theme.inset
                border.width: header.visualFocus ? 2 : 1
                border.color: header.lit || header.visualFocus ? Theme.gold : Theme.line
                Shape {
                    anchors.centerIn: parent
                    width: 10
                    height: 10
                    rotation: header.checked ? 90 : 0
                    Behavior on rotation { NumberAnimation { duration: Theme.quick; easing.type: Easing.OutCubic } }
                    ShapePath {
                        strokeColor: header.lit ? Theme.gold : Theme.text
                        strokeWidth: 1.5
                        fillColor: "transparent"
                        capStyle: ShapePath.RoundCap
                        joinStyle: ShapePath.RoundJoin
                        PathSvg { path: "M 3.5 1 L 7.5 5 L 3.5 9" }
                    }
                }
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: expander.title
                font.pixelSize: Theme.sizeSmall + 1
                font.weight: Font.DemiBold
                color: Theme.text
            }
        }
        background: null
    }

    ColumnLayout {
        id: body
        Layout.fillWidth: true
        Layout.topMargin: 8
        visible: expander.expanded
        spacing: 8
    }
}
