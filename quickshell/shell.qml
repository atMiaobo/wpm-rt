import Quickshell
import Quickshell.Io
import QtQuick

ShellRoot {
  id: root

  property int wpm: 0
  property real cps: 0
  property bool active: false
  property string errorText: ""

  function updateFromLine(line) {
    try {
      const stats = JSON.parse(line.trim());
      root.wpm = stats.wpm || 0;
      root.cps = stats.cps || 0;
      root.active = !!stats.active && root.wpm > 0;
    } catch (error) {
      console.log("wpm-rt: failed to parse monitor line:", line);
    }
  }

  Variants {
    model: Quickshell.screens

    PanelWindow {
      id: panel
      property var modelData

      screen: modelData
      color: "transparent"
      aboveWindows: true
      exclusiveZone: 0
      implicitWidth: root.errorText.length > 0 ? 260 : 118
      implicitHeight: root.errorText.length > 0 ? 82 : 72

      anchors {
        top: true
        right: true
      }

      margins {
        top: 26
        right: 26
      }

      Rectangle {
        id: card
        anchors.fill: parent
        radius: 8
        color: "#18181b"
        border.color: "#3f3f46"
        border.width: 1
        opacity: root.active || root.errorText.length > 0 ? 0.96 : 0
        scale: root.active || root.errorText.length > 0 ? 1 : 0.94

        Behavior on opacity {
          NumberAnimation { duration: 130; easing.type: Easing.OutCubic }
        }

        Behavior on scale {
          NumberAnimation { duration: 130; easing.type: Easing.OutCubic }
        }

        Text {
          anchors {
            horizontalCenter: parent.horizontalCenter
            top: parent.top
            topMargin: 11
          }
          visible: root.errorText.length === 0
          text: root.wpm
          color: "#f4f4f5"
          font.pixelSize: 28
          font.weight: Font.DemiBold
        }

        Text {
          anchors {
            horizontalCenter: parent.horizontalCenter
            bottom: parent.bottom
            bottomMargin: 12
          }
          visible: root.errorText.length === 0
          text: "WPM"
          color: "#a1a1aa"
          font.pixelSize: 12
          font.letterSpacing: 0
        }

        Text {
          anchors {
            fill: parent
            margins: 12
          }
          visible: root.errorText.length > 0
          text: root.errorText
          color: "#f4f4f5"
          font.pixelSize: 12
          wrapMode: Text.WordWrap
          horizontalAlignment: Text.AlignHCenter
          verticalAlignment: Text.AlignVCenter
        }
      }
    }
  }

  Process {
    command: ["@wpmRtBin@", "stream"]
    running: true

    stdout: SplitParser {
      onRead: data => root.updateFromLine(data)
    }

    stderr: SplitParser {
      onRead: data => root.errorText = data.trim()
    }
  }
}
