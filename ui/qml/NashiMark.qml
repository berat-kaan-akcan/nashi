import QtQuick

// The Nashi pear (resources/mark*.svg), `size` × `size` in layouts too.
Item {
    id: mark
    property real size: 44
    property bool happy: false
    implicitWidth: size
    implicitHeight: size
    Accessible.ignored: true

    Image {
        anchors.fill: parent
        sourceSize: Qt.size(mark.size * 2, mark.size * 2)
        smooth: true
        source: mark.happy ? "qrc:/qt/qml/NashiApp/resources/mark-happy.svg" : "qrc:/qt/qml/NashiApp/resources/mark.svg"
    }
}
