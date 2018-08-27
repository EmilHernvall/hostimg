import * as React from 'react';
import './App.css';
import Gallery from './model/Gallery';
import Image from './model/Image';
import Thumbnail from './Thumbnail';
import Lightbox from './Lightbox';

interface AppState {
    gallery: Gallery | null;
    path: string;
    width: number;
    lightboxImage: Image | null;
}

const MAX_HEIGHT = 200;

class App extends React.Component<any, AppState> {

  private ref: React.RefObject<any>;

  constructor(props: any) {
    super(props);
    this.state = {
      gallery: null,
      lightboxImage: null,
      path: window.location.hash.substring(1),
      width: 0,
    };
    this.ref = React.createRef();
  }

  public componentDidMount() {
    const width = this.ref.current.offsetWidth;
    this.setState({ width });

    this.loadGallery(this.state.path);

    window.addEventListener("hashchange", e => {
      const path = window.location.hash.substring(1);
      console.log("new hash: " + path);
      this.loadGallery(path);
    });
  }

  private buildRow(workingset: Image[], height: number) {
    const thumbnails = workingset.map(image => {
      const aspectRatio = image.width/image.height;
      const imageWidth = aspectRatio * height;
      return <Thumbnail
        key={ 'thumbnail-' + image.hash }
        image={ image }
        width={ imageWidth }
        height={ height }
        onClick={ img => this.onImageClick(img) }/>;
    });
    return thumbnails;
  }

  private generateThumbnailSet(): JSX.Element[] {
    const { width, gallery } = this.state;
    if (!gallery) {
      return [];
    }

    let rows: JSX.Element[] = [];

    let workingset: Image[] = [];
    let aspectSum = 0;
    const lastIdx  = gallery.images.length - 1;
    for (let [idx, image] of gallery.images.entries()) {
      aspectSum += image.width/image.height;
      workingset.push(image);

      const workingsetHeight = width / aspectSum;
      if (workingsetHeight <= MAX_HEIGHT) {
        const thumbnails = this.buildRow(workingset, workingsetHeight);
        rows.push(<div className="row" key={ 'row' + rows.length }>{ thumbnails }</div>);

        workingset = [];
        aspectSum = 0;
      } else if (idx == lastIdx) {
        const thumbnails = this.buildRow(workingset, MAX_HEIGHT);
        rows.push(<div className="row" key={ 'row' + rows.length }>{ thumbnails }</div>);
      }
    }

    return rows;
  }

  private directoriesFromGallery() {
    const { path, gallery } = this.state;
    if (!gallery) {
      return [];
    }

    const extraEntries = [];
    if (path != "") {
      const pathEntries = path.split(/\//);
      pathEntries.pop();
      const parentPath = pathEntries.join("/");
      extraEntries.push({
          path: parentPath,
          name: ".."
      });
    }

    const directories = extraEntries.concat(gallery.sub_galleries);

    return directories.map((dir, idx) => {
      return <li key={ `dir${idx}` }>
        <a href="javascript:;" onClick={ (e) => { window.location.hash = dir.path; } }>{ dir.name }</a>
      </li>;
    });
  }

  private loadGallery(path: string) {
    fetch("http://localhost:1080/gallery/" + path)
      .then(result => result.json())
      .then(gallery => {
          console.log("loaded " + path);
          this.setState({ path, gallery })
      });
  }

  private onImageClick(image: Image) {
    this.setState({ lightboxImage: image });
  }

  private changeLightboxImage(offset: number) {
    const { gallery, lightboxImage } = this.state;
    if (!gallery || !lightboxImage) {
      return;
    }
    const images = gallery.images;

    const newPos = (images.indexOf(lightboxImage) + offset + images.length) % images.length;

    this.setState({ lightboxImage: images[newPos] });
  }

  public render() {
    const { lightboxImage } = this.state;

    const thumbnails = this.generateThumbnailSet();
    const directories = this.directoriesFromGallery();

    const lightbox = lightboxImage ? <Lightbox
        image={ lightboxImage }
        onNextImage={ () => this.changeLightboxImage(1) }
        onPreviousImage={ () => this.changeLightboxImage(-1) }
        onClose={ () => this.setState({ lightboxImage: null }) }/> : null;

    return (
      <div className="App">
        <div className="directories">
          <ul>
            { directories }
          </ul>
        </div>
        <div className="images" ref={ this.ref }>
          { thumbnails }
        </div>
        { lightbox }
      </div>
    );
  }
}

export default App;
