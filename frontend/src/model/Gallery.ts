import Image from './Image';

export interface SubGallery {
    name: string;
    path: string;
}

export default interface Gallery {
    name: string;
    images: Image[];
    sub_galleries: SubGallery[];
}

