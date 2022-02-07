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

    func setupAndStartAnimation()
    {
        let saverName = UserDefaults.standard.string(forKey: "saver") ?? "Flux"
        guard let saverBundle = loadSaverBundle(saverName) else {
            NSLog("Can't find or load bundle for saver named \(saverName).")
            return
        }
        let saverClass = saverBundle.principalClass! as! ScreenSaverView.Type
        
        view = saverClass.init(frame: window.contentView!.frame, isPreview: false)
        view.autoresizingMask = [NSView.AutoresizingMask.width, NSView.AutoresizingMask.height]

        window.backingType = saverClass.backingStoreType()
        window.title = view.className
        window.contentView!.autoresizesSubviews = true
        window.contentView!.addSubview(view)

        view.startAnimation()
    }

    private func loadSaverBundle(_ name: String) -> Bundle?
    {
        let myBundle = Bundle(for: AppDelegate.self)
        let saverBundleURL = myBundle.bundleURL.deletingLastPathComponent().appendingPathComponent("\(name).saver", isDirectory: true)
        Swift.print(saverBundleURL)
        let saverBundle = Bundle(url: saverBundleURL)
        saverBundle?.load()
        return saverBundle
    }

    func restartAnimation()
    {
        if view.isAnimating {
            view.stopAnimation()
        }
        view.startAnimation()
    }

    @IBAction func showPreferences(_ sender: NSObject!)
    {
        window.beginSheet(view.configureSheet!, completionHandler: nil)
    }

}

extension AppDelegate: NSApplicationDelegate
{
    func applicationDidFinishLaunching(_ aNotification: Notification)
    {
        setupAndStartAnimation()
    }
}


extension AppDelegate: NSWindowDelegate
{
    func windowWillClose(_ notification: Notification)
    {
        NSApplication.shared.terminate(window)
    }

    func windowDidResize(_ notification: Notification)
    {
    }

    func windowDidEndSheet(_ notification: Notification)
    {
        restartAnimation()
    }
}

//    func applicationWillTerminate(_ aNotification: Notification) {
//        // Insert code here to tear down your application
//    }
//
//    func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
//        return true
//    }
