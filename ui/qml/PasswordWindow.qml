pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

// Asks for the password of an archive or PDF whose job failed with password_required,
// then runs the job again (`nashi-cli --password-stdin`). The typed text is
// cleared as soon as it has been handed on or the window closes; it is never
// stored, logged or put on a command line.
ApplicationWindow {
    id: root
    objectName: "passwordWindow"

    property JobRunner job: null
    property string fileName: ""
    // A retry that failed again means the password did not open it.
    property bool retried: false

    readonly property bool canSubmit: job !== null && passwordField.text.length > 0

    function openFor(failedJob, name, wasRetry) {
        job = failedJob
        fileName = name
        retried = Boolean(wasRetry)
        passwordField.clear()
        passwordField.forceActiveFocus()
    }

    function submit() {
        if (!canSubmit)
            return
        const typed = passwordField.text
        passwordField.clear()
        App.jobs.retryWithPassword(job, typed)
        job = null
        root.close()
    }

    visible: false
    width: 420
    height: Math.max(220, contentCol.implicitHeight + 48)
    minimumWidth: 360
    minimumHeight: 200
    title: qsTr("Password Required")
    color: Theme.canvas
    flags: Qt.Dialog

    palette.window: Theme.canvas
    palette.windowText: Theme.text
    palette.base: Theme.surface
    palette.text: Theme.text
    palette.highlight: Theme.gold
    palette.highlightedText: Theme.buttonInk
    palette.placeholderText: Theme.tertiaryText
    palette.mid: Theme.border

    onVisibleChanged: if (!visible) { passwordField.clear(); job = null }
    // The job card it belongs to was removed (cleared or replaced).
    onJobChanged: if (job === null && visible) root.close()

    Shortcut { sequence: "Escape"; onActivated: root.close() }
    Shortcut {
        sequences: ["Return", "Enter"]
        enabled: root.canSubmit
        onActivated: root.submit()
    }

    ColumnLayout {
        id: contentCol
        anchors.fill: parent
        anchors.margins: 24
        spacing: 12

        Text {
            text: qsTr("PROTECTED FILE")
            font.pixelSize: Theme.sizeTiny
            font.weight: Font.Bold
            font.letterSpacing: 1.4
            color: Theme.goldInk
        }
        Text {
            Layout.fillWidth: true
            text: root.fileName
            elide: Text.ElideMiddle
            font.pixelSize: Theme.sizeHeading
            font.weight: Font.Bold
            color: Theme.text
        }
        Text {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            font.pixelSize: Theme.sizeBody
            color: root.retried ? Theme.danger : Theme.secondaryText
            text: root.retried ? qsTr("That password did not open it. Check it and try again.")
                               : qsTr("Enter the password to open this file. Nashi does not save it.")
        }

        TextField {
            id: passwordField
            objectName: "filePasswordField"
            Layout.fillWidth: true
            echoMode: TextInput.Password
            inputMethodHints: Qt.ImhHiddenText | Qt.ImhSensitiveData | Qt.ImhNoPredictiveText | Qt.ImhNoAutoUppercase
            placeholderText: qsTr("Password")
            selectByMouse: true
            Accessible.name: qsTr("File password")
        }

        Item { Layout.fillHeight: true }

        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Item { Layout.fillWidth: true }
            NButton {
                kind: "quiet"
                text: qsTr("Cancel")
                onClicked: root.close()
            }
            NButton {
                objectName: "filePasswordContinueButton"
                kind: "primary"
                iconName: "go-next"
                text: qsTr("Continue")
                enabled: root.canSubmit
                onClicked: root.submit()
            }
        }
    }
}
