pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

// Pick where the window you were just using goes (KDE Plasma, via KWin).
ApplicationWindow {
    id: root

    title: qsTr("Place window")
    width: 420
    height: content.implicitHeight + 40
    minimumWidth: 360
    color: Theme.canvas

    readonly property var titles: ({
        "left": qsTr("Left half"),
        "right": qsTr("Right half"),
        "top": qsTr("Top half"),
        "bottom": qsTr("Bottom half"),
        "topLeft": qsTr("Top left"),
        "topRight": qsTr("Top right"),
        "bottomLeft": qsTr("Bottom left"),
        "bottomRight": qsTr("Bottom right"),
        "leftThird": qsTr("Left third"),
        "centerThird": qsTr("Middle third"),
        "rightThird": qsTr("Right third"),
        "maximize": qsTr("Fill the screen")
    })

    property string note: ""

    function choose(layout) {
        if (App.windowPlacer.place(layout))
            root.close()
        else
            note = qsTr("KWin did not accept the request. This works on KDE Plasma only.")
    }

    onVisibleChanged: if (visible) note = App.windowPlacer.available()
                                 ? "" : qsTr("KWin was not found. This works on KDE Plasma only.")

    Shortcut {
        sequences: [StandardKey.Cancel]
        onActivated: root.close()
    }

    ColumnLayout {
        id: content
        anchors.fill: parent
        anchors.margins: 20
        spacing: 12

        Text {
            Layout.fillWidth: true
            text: qsTr("Where should the last window you used go?")
            wrapMode: Text.WordWrap
            font.pixelSize: Theme.sizeBody
            color: Theme.text
        }

        GridLayout {
            columns: 4
            columnSpacing: 10
            rowSpacing: 10
            Layout.fillWidth: true

            Repeater {
                model: App.windowPlacer.layoutIds()
                delegate: AbstractButton {
                    id: tile
                    required property string modelData
                    readonly property var frac: App.windowPlacer.layoutFractions(modelData)
                    objectName: "place-" + modelData
                    Layout.fillWidth: true
                    Layout.preferredHeight: 84
                    hoverEnabled: true
                    onClicked: root.choose(modelData)
                    Accessible.name: root.titles[modelData]
                    ToolTip.visible: hovered
                    ToolTip.text: root.titles[modelData]

                    background: Rectangle {
                        radius: Theme.radiusRow
                        color: tile.hovered || tile.activeFocus ? Theme.hover : "transparent"
                        border.width: 1
                        border.color: tile.activeFocus ? Theme.forest : Theme.border
                    }
                    contentItem: Item {
                        Rectangle {
                            id: screen
                            anchors.centerIn: parent
                            width: Math.min(parent.width - 16, 76)
                            height: Math.round(width * 0.62)
                            radius: 3
                            color: "transparent"
                            border.width: 1
                            border.color: Theme.secondaryText
                            Rectangle {
                                x: tile.frac.length ? tile.frac[0] * screen.width + 1 : 0
                                y: tile.frac.length ? tile.frac[1] * screen.height + 1 : 0
                                width: tile.frac.length ? tile.frac[2] * screen.width - 2 : 0
                                height: tile.frac.length ? tile.frac[3] * screen.height - 2 : 0
                                radius: 2
                                color: Theme.forest
                                opacity: 0.85
                            }
                        }
                    }
                }
            }
        }

        Text {
            Layout.fillWidth: true
            visible: root.note.length > 0
            text: root.note
            wrapMode: Text.WordWrap
            font.pixelSize: Theme.sizeSmall
            color: Theme.secondaryText
        }
    }
}
