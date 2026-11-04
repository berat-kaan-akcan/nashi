pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

// Native KDE Purpose ("Export") share dialog, dynamically instantiated at
// runtime so minimal environments without Purpose have zero QML import deps.
ApplicationWindow {
    id: root

    property var paths: []
    property var purposeView: null
    property int purposeDepth: 1
    property string errorMessage: ""
    property bool loadFailed: false

    function clearPurpose() {
        if (purposeView) {
            try {
                purposeView.destroy()
            } catch (e) {
            }
            purposeView = null
        }
        purposeDepth = 1
    }

    function activateAlternative(rowIndex) {
        if (!purposeView)
            return
        try {
            purposeView.createJob(rowIndex)
        } catch (e) {
            errorMessage = String(e)
        }
    }

    function resetView() {
        if (!purposeView)
            return
        try {
            purposeView.reset()
        } catch (e) {
        }
    }

    function share(filePaths) {
        clearPurpose()
        errorMessage = ""
        loadFailed = false

        const arr = []
        if (filePaths) {
            for (let i = 0; i < filePaths.length; ++i) {
                const s = String(filePaths[i]).trim()
                if (s.length > 0)
                    arr.push(s)
            }
        }
        paths = arr
        if (arr.length === 0 || !App.shareAvailable)
            return

        try {
            const qml = 'import QtQuick\n'
                      + 'import org.kde.purpose\n'
                      + 'AlternativesView {\n'
                      + '    anchors.fill: parent\n'
                      + '    pluginType: "Export"\n'
                      + '}\n'
            const view = Qt.createQmlObject(qml, viewHost, "NashiPurposeView")
            if (!view) {
                loadFailed = true
                return
            }
            view.delegate = alternativeDelegate
            view.inputData = {
                "urls": App.fileUrlsFor(arr),
                "mimeType": App.commonMimeType(arr)
            }
            view.depthChanged.connect(function() {
                if (root.purposeView === view)
                    root.purposeDepth = view.depth
            })
            view.finished.connect(function(output, error, message) {
                const outUrl = (output && output.url) ? String(output.url) : ""
                App.reportShareResult(error, message || "", outUrl, root.paths.length)
                if (error === 0) {
                    root.close()
                } else {
                    root.errorMessage = message && String(message).length > 0
                                        ? String(message) : qsTr("Sharing failed.")
                    root.resetView()
                }
            })
            purposeView = view
            purposeDepth = view.depth
        } catch (e) {
            loadFailed = true
            errorMessage = String(e)
        }
    }

    visible: false
    width: 440
    height: 480
    minimumWidth: 380
    minimumHeight: 380
    flags: Qt.Dialog
    title: qsTr("Share")
    color: Theme.canvas

    palette.window: Theme.canvas
    palette.windowText: Theme.text
    palette.base: Theme.surface
    palette.alternateBase: Theme.inset
    palette.text: Theme.text
    palette.button: Theme.surface
    palette.buttonText: Theme.text
    palette.highlight: Theme.gold
    palette.highlightedText: Theme.buttonInk
    palette.placeholderText: Theme.tertiaryText
    palette.mid: Theme.border

    onClosing: clearPurpose()

    Shortcut {
        sequence: "Escape"
        onActivated: {
            if (root.purposeDepth > 1)
                root.resetView()
            else
                root.close()
        }
    }

    Component {
        id: alternativeDelegate

        Item {
            id: altRow
            required property string display
            required property int index

            width: ListView.view ? ListView.view.width : 360
            height: 52

            Rectangle {
                anchors.fill: parent
                anchors.bottomMargin: 6
                radius: Theme.radiusCompact + 2
                color: rowHover.hovered ? Theme.inset : Theme.canvas
                border.width: 1
                border.color: Theme.border

                HoverHandler { id: rowHover }
                TapHandler { onTapped: root.activateAlternative(altRow.index) }

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 10
                    spacing: 10

                    Rectangle {
                        Layout.preferredWidth: 28
                        Layout.preferredHeight: 28
                        radius: 8
                        color: Theme.sage
                        Image {
                            anchors.centerIn: parent
                            source: "image://icon/document-send"
                            sourceSize: Qt.size(32, 32)
                            width: 16
                            height: 16
                        }
                    }

                    Text {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: altRow.display
                        font.pixelSize: Theme.sizeBody
                        font.weight: Font.Medium
                        color: Theme.text
                    }

                    NButton {
                        compact: true
                        kind: "secondary"
                        text: qsTr("Choose")
                        onClicked: root.activateAlternative(altRow.index)
                    }
                }
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 20
        spacing: 12

        RowLayout {
            Layout.fillWidth: true
            spacing: 10

            NButton {
                visible: root.purposeDepth > 1
                compact: true
                kind: "secondary"
                iconName: "go-previous"
                text: qsTr("Back")
                onClicked: root.resetView()
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 3
                Pill {
                    text: qsTr("SHARE")
                    iconName: "document-send"
                    tint: Theme.sky
                }
                Text {
                    Layout.fillWidth: true
                    elide: Text.ElideMiddle
                    font.pixelSize: Theme.sizeTitle - 3
                    font.weight: Font.Bold
                    color: Theme.text
                    text: root.paths.length === 0 ? qsTr("Share files")
                          : root.paths.length === 1 ? App.fileName(String(root.paths[0]))
                          : qsTr("Share %1 files").arg(root.paths.length)
                }
            }

            NashiMark {
                size: 42
                happy: root.paths.length > 0 && root.errorMessage.length === 0
            }
        }

        Text {
            Layout.fillWidth: true
            visible: root.errorMessage.length > 0
            wrapMode: Text.Wrap
            font.pixelSize: Theme.sizeSmall
            color: Theme.danger
            text: root.errorMessage
        }

        Card {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true

            Item {
                id: viewHost
                objectName: "sharePurposeHost"
                anchors.fill: parent
                anchors.margins: 12
                visible: root.purposeView !== null
            }

            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(320, parent.width - 36)
                visible: root.purposeView === null
                spacing: 10

                Text {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                    font.pixelSize: Theme.sizeBody
                    color: Theme.secondaryText
                    text: !App.shareAvailable || root.loadFailed
                          ? qsTr("KDE Purpose sharing is not available in this environment.")
                          : qsTr("Pick one or more files to share with nearby devices or apps.")
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Text {
                Layout.fillWidth: true
                font.pixelSize: Theme.sizeTiny
                color: Theme.tertiaryText
                elide: Text.ElideMiddle
                text: root.paths.length > 0 ? App.commonMimeType(root.paths) : ""
            }
            NButton {
                kind: "quiet"
                text: qsTr("Close")
                onClicked: root.close()
            }
        }
    }
}
