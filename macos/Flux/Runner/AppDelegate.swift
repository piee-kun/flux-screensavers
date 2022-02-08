//
//  AppDelegate.swift
//  Runner
//
//  Created by Sander Melnikov on 07/02/2022.
//

import Cocoa
import ScreenSaver

@main
class AppDelegate: NSObject {

    @IBOutlet var window: NSWindow!
    
    var view: ScreenSaverView!

    func setupAndStartAnimation() {
        let saverName = UserDefaults.standard.string(forKey: "saver") ?? "Flux"
        guard let saverBundle = loadSaverBundle(saverName) else {
            NSLog("Can't find or load bundle for saver named \(saverName).")
            return
        }
        let saverClass = saverBundle.principalClass! as! ScreenSaverView.Type
        
        view = saverClass.init(frame: window.contentView!.frame, isPreview: false)

        window.backingType = saverClass.backingStoreType()
        window.title = "Flux"
        // Don’t autoresize! We need to “detach” the view from OpenGL first.
        window.contentView!.autoresizesSubviews = false
        window.contentView!.addSubview(view)

        view.startAnimation()
    }

    private func loadSaverBundle(_ name: String) -> Bundle? {
        let myBundle = Bundle(for: AppDelegate.self)
        let saverBundleURL = myBundle.bundleURL.deletingLastPathComponent().appendingPathComponent("\(name).saver", isDirectory: true)
//        let saverBundleURL = URL(fileURLWithPath: "/System/Library/Screen Savers/Drift.saver", isDirectory: true)
        let saverBundle = Bundle(url: saverBundleURL)
        saverBundle?.load()
        return saverBundle
    }

    func restartAnimation() {
        if view.isAnimating {
            view.stopAnimation()
        }
        view.startAnimation()
    }

    @IBAction func showPreferences(_ sender: NSObject!) {
        window.beginSheet(view.configureSheet!, completionHandler: nil)
    }
}

extension AppDelegate: NSApplicationDelegate {
    func applicationDidFinishLaunching(_ aNotification: Notification) {
        // Watch for resize events
        NotificationCenter.default.addObserver(self, selector: #selector(NSWindowDelegate.windowDidResize(_:)), name: NSWindow.didResizeNotification, object: window)
        
        setupAndStartAnimation()
    }
    
    //    func applicationWillTerminate(_ aNotification: Notification) {
    //        // Insert code here to tear down your application
    //    }
}


extension AppDelegate: NSWindowDelegate {
    func windowWillClose(_ notification: Notification) {
        NSApplication.shared.terminate(window)
    }

    func windowDidResize(_ notification: Notification) {
        let window = notification.object as! NSWindow
        // TODO: whats the old size?
        view.resizeSubviews(withOldSize: window.frame.size)
    }

    func windowDidEndSheet(_ notification: Notification) {
        restartAnimation()
    }
}
